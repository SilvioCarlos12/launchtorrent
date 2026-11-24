use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub download_dir: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_file_path();
        let mut cfg = match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        };
        // Drive roots (e.g. D:\) often break session/persistence writes; use a subfolder.
        let normalized = normalize_download_dir(&cfg.download_dir);
        if normalized != cfg.download_dir {
            cfg.download_dir = normalized;
            let _ = cfg.save();
        }
        cfg
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    pub fn ensure_download_dir(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.download_dir)?;
        // Probe writability — create_dir_all on a drive root can "succeed" without write access.
        let probe = self.download_dir.join(".launchtorrent_write_test");
        match std::fs::write(&probe, b"ok") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "sem permissão de escrita em {}: {e}",
                self.download_dir.display()
            )),
        }
    }
}

/// If `path` is a Windows/Unix drive/volume root, append `LaunchTorrent`.
pub fn normalize_download_dir(path: &Path) -> PathBuf {
    if is_volume_root(path) {
        path.join("LaunchTorrent")
    } else {
        path.to_path_buf()
    }
}

fn is_volume_root(path: &Path) -> bool {
    let comps: Vec<_> = path.components().collect();
    match comps.as_slice() {
        [Component::RootDir] => true,
        [Component::Prefix(_), Component::RootDir] => true,
        _ => false,
    }
}

pub fn default_download_dir() -> PathBuf {
    dirs::download_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Downloads")
        })
        .join("LaunchTorrent")
}

pub fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("LaunchTorrent")
        .join("config.toml")
}

pub fn path_display(path: &Path) -> String {
    path.display().to_string()
}
