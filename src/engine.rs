use bytes::Bytes;
use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, TorrentStatsState};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// How long to wait for magnet metadata (DHT/peers) before failing visibly.
pub const MAGNET_RESOLVE_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone)]
pub enum EngineCommand {
    AddMagnet(String),
    AddTorrentFile(PathBuf),
    Pause(usize),
    Resume(usize),
    Remove { id: usize, delete_files: bool },
    SetDownloadDir(PathBuf),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TorrentUiState {
    Initializing,
    Downloading,
    Paused,
    Finished,
    Error,
}

#[derive(Debug, Clone)]
pub struct TorrentSnapshot {
    pub id: usize,
    pub name: String,
    pub progress: f32,
    pub progress_bytes: u64,
    pub total_bytes: u64,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub peers: u32,
    pub state: TorrentUiState,
    pub error: Option<String>,
}

/// Magnet/.torrent still resolving metadata — not yet in the librqbit session.
#[derive(Debug, Clone)]
pub struct PendingAdd {
    pub id: u64,
    pub label: String,
    pub kind: PendingKind,
    pub started_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKind {
    Magnet,
    TorrentFile,
}

#[derive(Debug, Clone, Default)]
pub struct EngineSnapshot {
    pub torrents: Vec<TorrentSnapshot>,
    pub pending: Vec<PendingAdd>,
    pub last_error: Option<String>,
    pub ready: bool,
}

pub struct EngineHandle {
    pub cmd_tx: mpsc::UnboundedSender<EngineCommand>,
    pub snapshot_rx: watch::Receiver<EngineSnapshot>,
}

impl EngineHandle {
    pub fn send(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
    }
}

enum JobResult {
    AddFinished {
        pending_id: u64,
        result: Result<(), String>,
    },
}

pub fn spawn_engine(download_dir: PathBuf) -> EngineHandle {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (snapshot_tx, snapshot_rx) = watch::channel(EngineSnapshot::default());

    std::thread::Builder::new()
        .name("launchtorrent-engine".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("lt-tokio")
                .build()
                .expect("failed to build tokio runtime");
            rt.block_on(engine_loop(download_dir, cmd_rx, snapshot_tx));
        })
        .expect("failed to spawn engine thread");

    EngineHandle {
        cmd_tx,
        snapshot_rx,
    }
}

async fn engine_loop(
    mut download_dir: PathBuf,
    mut cmd_rx: mpsc::UnboundedReceiver<EngineCommand>,
    snapshot_tx: watch::Sender<EngineSnapshot>,
) {
    let mut last_error: Option<String> = None;
    let mut pending: Vec<PendingAdd> = Vec::new();
    let mut next_pending_id: u64 = 1;
    let (job_tx, mut job_rx) = mpsc::unbounded_channel::<JobResult>();

    let session = match Session::new(download_dir.clone()).await {
        Ok(s) => {
            let _ = snapshot_tx.send(EngineSnapshot {
                torrents: Vec::new(),
                pending: Vec::new(),
                last_error: None,
                ready: true,
            });
            s
        }
        Err(e) => {
            let msg = format!("Falha ao iniciar sessão BitTorrent: {e:#}");
            tracing::error!("{msg}");
            let _ = snapshot_tx.send(EngineSnapshot {
                torrents: Vec::new(),
                pending: Vec::new(),
                last_error: Some(msg.clone()),
                ready: false,
            });
            // Accept commands so the UI can keep showing a clear error instead of
            // silently dropping Adicionar clicks.
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    EngineCommand::Shutdown => break,
                    EngineCommand::AddMagnet(_) | EngineCommand::AddTorrentFile(_) => {
                        let _ = snapshot_tx.send(EngineSnapshot {
                            torrents: Vec::new(),
                            pending: Vec::new(),
                            last_error: Some(format!(
                                "Motor BitTorrent não iniciou — não dá para adicionar. {msg}"
                            )),
                            ready: false,
                        });
                    }
                    EngineCommand::SetDownloadDir(dir) => {
                        if let Err(e) = std::fs::create_dir_all(&dir) {
                            let _ = snapshot_tx.send(EngineSnapshot {
                                torrents: Vec::new(),
                                pending: Vec::new(),
                                last_error: Some(format!(
                                    "Pasta de download inválida ({e}). Reinicie após corrigir a pasta. Sessão: {msg}"
                                )),
                                ready: false,
                            });
                        } else {
                            let _ = snapshot_tx.send(EngineSnapshot {
                                torrents: Vec::new(),
                                pending: Vec::new(),
                                last_error: Some(format!(
                                    "Pasta atualizada, mas a sessão já falhou ao iniciar. Reinicie o app. ({msg})"
                                )),
                                ready: false,
                            });
                        }
                    }
                    _ => {
                        let _ = snapshot_tx.send(EngineSnapshot {
                            torrents: Vec::new(),
                            pending: Vec::new(),
                            last_error: Some(msg.clone()),
                            ready: false,
                        });
                    }
                }
            }
            return;
        }
    };

    let mut tick = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    None | Some(EngineCommand::Shutdown) => {
                        session.stop().await;
                        break;
                    }
                    Some(EngineCommand::AddMagnet(url)) => {
                        last_error = None;
                        let url = url.trim().to_string();
                        let pending_id = next_pending_id;
                        next_pending_id += 1;
                        let label = magnet_display_name(&url);
                        pending.push(PendingAdd {
                            id: pending_id,
                            label,
                            kind: PendingKind::Magnet,
                            started_at: std::time::Instant::now(),
                        });
                        let session = Arc::clone(&session);
                        let dir = download_dir.clone();
                        let job_tx = job_tx.clone();
                        tokio::spawn(async move {
                            let result = match tokio::time::timeout(
                                MAGNET_RESOLVE_TIMEOUT,
                                add_magnet(&session, &url, &dir),
                            )
                            .await
                            {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(e)) => Err(format!("Magnet: {e:#}")),
                                Err(_) => Err(format!(
                                    "Magnet: tempo esgotado ({}s) ao obter metadados — sem peers/DHT/trackers respondendo.",
                                    MAGNET_RESOLVE_TIMEOUT.as_secs()
                                )),
                            };
                            let _ = job_tx.send(JobResult::AddFinished { pending_id, result });
                        });
                    }
                    Some(EngineCommand::AddTorrentFile(path)) => {
                        last_error = None;
                        let pending_id = next_pending_id;
                        next_pending_id += 1;
                        let label = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("arquivo.torrent")
                            .to_string();
                        pending.push(PendingAdd {
                            id: pending_id,
                            label,
                            kind: PendingKind::TorrentFile,
                            started_at: std::time::Instant::now(),
                        });
                        let session = Arc::clone(&session);
                        let dir = download_dir.clone();
                        let job_tx = job_tx.clone();
                        tokio::spawn(async move {
                            let result = add_torrent_file(&session, &path, &dir)
                                .await
                                .map_err(|e| format!("Arquivo .torrent: {e:#}"));
                            let _ = job_tx.send(JobResult::AddFinished { pending_id, result });
                        });
                    }
                    Some(EngineCommand::Pause(id)) => {
                        if let Err(e) = pause_torrent(&session, id).await {
                            last_error = Some(format!("Pausar: {e:#}"));
                        }
                    }
                    Some(EngineCommand::Resume(id)) => {
                        if let Err(e) = resume_torrent(&session, id).await {
                            last_error = Some(format!("Retomar: {e:#}"));
                        }
                    }
                    Some(EngineCommand::Remove { id, delete_files }) => {
                        if let Err(e) = session.delete(TorrentIdOrHash::Id(id), delete_files).await {
                            last_error = Some(format!("Remover: {e:#}"));
                        }
                    }
                    Some(EngineCommand::SetDownloadDir(dir)) => {
                        if let Err(e) = std::fs::create_dir_all(&dir) {
                            last_error = Some(format!("Pasta de download: {e}"));
                        } else {
                            download_dir = dir;
                            last_error = None;
                        }
                    }
                }
                publish_snapshot(&session, &snapshot_tx, &pending, last_error.clone());
            }
            job = job_rx.recv() => {
                if let Some(JobResult::AddFinished { pending_id, result }) = job {
                    pending.retain(|p| p.id != pending_id);
                    match result {
                        Ok(()) => {
                            last_error = None;
                        }
                        Err(e) => {
                            tracing::warn!("{e}");
                            last_error = Some(e);
                        }
                    }
                    publish_snapshot(&session, &snapshot_tx, &pending, last_error.clone());
                }
            }
            _ = tick.tick() => {
                publish_snapshot(&session, &snapshot_tx, &pending, last_error.clone());
            }
        }
    }
}

fn magnet_display_name(url: &str) -> String {
    // Prefer dn= from magnet; fall back to short hash.
    if let Some(rest) = url.split("dn=").nth(1) {
        let raw = rest.split('&').next().unwrap_or(rest);
        if let Ok(decoded) = urlencoding_lite(raw) {
            if !decoded.is_empty() {
                return decoded;
            }
        }
        if !raw.is_empty() {
            return raw.to_string();
        }
    }
    if let Some(hash) = url.split("btih:").nth(1) {
        let hash = hash.split('&').next().unwrap_or(hash);
        let short = if hash.len() > 8 { &hash[..8] } else { hash };
        return format!("magnet:{short}…");
    }
    "magnet…".to_string()
}

/// Minimal percent-decoding for magnet `dn=` (no extra crate).
fn urlencoding_lite(s: &str) -> Result<String, ()> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| ())?;
                let v = u8::from_str_radix(h, 16).map_err(|_| ())?;
                out.push(v);
                i += 3;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

fn add_opts(download_dir: &PathBuf) -> AddTorrentOptions {
    AddTorrentOptions {
        overwrite: true,
        output_folder: Some(download_dir.display().to_string()),
        ..Default::default()
    }
}

async fn add_magnet(
    session: &Arc<Session>,
    url: &str,
    download_dir: &PathBuf,
) -> anyhow::Result<()> {
    let url = url.trim();
    anyhow::ensure!(!url.is_empty(), "link magnet vazio");
    let resp = session
        .add_torrent(AddTorrent::from_url(url), Some(add_opts(download_dir)))
        .await?;
    ensure_added(resp)?;
    Ok(())
}

async fn add_torrent_file(
    session: &Arc<Session>,
    path: &PathBuf,
    download_dir: &PathBuf,
) -> anyhow::Result<()> {
    let bytes = std::fs::read(path)?;
    let resp = session
        .add_torrent(
            AddTorrent::TorrentFileBytes(Bytes::from(bytes)),
            Some(add_opts(download_dir)),
        )
        .await?;
    ensure_added(resp)?;
    Ok(())
}

fn ensure_added(resp: AddTorrentResponse) -> anyhow::Result<()> {
    match resp {
        AddTorrentResponse::AlreadyManaged(_, _) => Ok(()),
        AddTorrentResponse::ListOnly(_) => anyhow::bail!("torrent em modo list-only"),
        other => {
            let _ = other
                .into_handle()
                .ok_or_else(|| anyhow::anyhow!("sem handle"))?;
            Ok(())
        }
    }
}

async fn pause_torrent(session: &Arc<Session>, id: usize) -> anyhow::Result<()> {
    let handle = session
        .get(TorrentIdOrHash::Id(id))
        .ok_or_else(|| anyhow::anyhow!("torrent {id} não encontrado"))?;
    session.pause(&handle).await?;
    Ok(())
}

async fn resume_torrent(session: &Arc<Session>, id: usize) -> anyhow::Result<()> {
    let handle = session
        .get(TorrentIdOrHash::Id(id))
        .ok_or_else(|| anyhow::anyhow!("torrent {id} não encontrado"))?;
    session.unpause(&handle).await?;
    Ok(())
}

fn publish_snapshot(
    session: &Arc<Session>,
    snapshot_tx: &watch::Sender<EngineSnapshot>,
    pending: &[PendingAdd],
    last_error: Option<String>,
) {
    let torrents = session.with_torrents(|iter| {
        iter.map(|(id, handle)| {
            let stats = handle.stats();
            let name = handle.name().unwrap_or_else(|| format!("torrent-{id}"));

            let (download_bps, upload_bps, peers) = if let Some(live) = &stats.live {
                let peers = live.snapshot.peer_stats.live_tcp
                    + live.snapshot.peer_stats.live_utp
                    + live.snapshot.peer_stats.live_socks;
                (
                    live.download_speed.as_bytes(),
                    live.upload_speed.as_bytes(),
                    peers as u32,
                )
            } else {
                (0, 0, 0)
            };

            let progress = if stats.total_bytes == 0 {
                if stats.finished {
                    1.0
                } else {
                    0.0
                }
            } else {
                (stats.progress_bytes as f64 / stats.total_bytes as f64).clamp(0.0, 1.0) as f32
            };

            let state = match stats.state {
                TorrentStatsState::Initializing { .. } => TorrentUiState::Initializing,
                TorrentStatsState::Paused => TorrentUiState::Paused,
                TorrentStatsState::Error => TorrentUiState::Error,
                TorrentStatsState::Live => {
                    if stats.finished {
                        TorrentUiState::Finished
                    } else {
                        TorrentUiState::Downloading
                    }
                }
            };

            TorrentSnapshot {
                id,
                name,
                progress,
                progress_bytes: stats.progress_bytes,
                total_bytes: stats.total_bytes,
                download_bps,
                upload_bps,
                peers,
                state,
                error: stats.error,
            }
        })
        .collect()
    });

    let _ = snapshot_tx.send(EngineSnapshot {
        torrents,
        pending: pending.to_vec(),
        last_error,
        ready: true,
    });
}
