use crate::config::{self, AppConfig};
use crate::engine::{
    spawn_engine, EngineCommand, EngineHandle, EngineSnapshot, PendingAdd, PendingKind,
    TorrentSnapshot, TorrentUiState, MAGNET_RESOLVE_TIMEOUT,
};
use crate::theme::{self, colors, ButtonKind, Icon};
use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Margin, RichText, Stroke, StrokeKind,
};
use std::path::PathBuf;
use std::time::Instant;

const MAGNET_EDIT_ID: &str = "launchtorrent_magnet_input";
const SIDEBAR_WIDTH: f32 = 236.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    All,
    Downloading,
    Paused,
    Finished,
    Error,
}

impl Filter {
    const ALL: [Filter; 5] = [
        Filter::All,
        Filter::Downloading,
        Filter::Paused,
        Filter::Finished,
        Filter::Error,
    ];

    fn label(self) -> &'static str {
        match self {
            Filter::All => "Todos",
            Filter::Downloading => "Baixando",
            Filter::Paused => "Pausados",
            Filter::Finished => "Concluídos",
            Filter::Error => "Com erro",
        }
    }

    fn dot(self) -> Color32 {
        match self {
            Filter::All => colors::SIDEBAR_MUTED,
            Filter::Downloading => colors::TEAL_DOT,
            Filter::Paused => colors::GRAY,
            Filter::Finished => colors::GREEN_DOT,
            Filter::Error => colors::RED_DOT,
        }
    }

    fn matches_torrent(self, state: &TorrentUiState) -> bool {
        match self {
            Filter::All => true,
            Filter::Downloading => {
                matches!(
                    state,
                    TorrentUiState::Downloading | TorrentUiState::Initializing
                )
            }
            Filter::Paused => *state == TorrentUiState::Paused,
            Filter::Finished => *state == TorrentUiState::Finished,
            Filter::Error => *state == TorrentUiState::Error,
        }
    }

    /// Torrents still resolving metadata count as "downloading".
    fn matches_pending(self) -> bool {
        matches!(self, Filter::All | Filter::Downloading)
    }
}

/// Per-state palette for a torrent row.
struct StateStyle {
    label: &'static str,
    icon: Icon,
    pill_bg: Color32,
    pill_fg: Color32,
    icon_bg: Color32,
    icon_fg: Color32,
    bar: Color32,
    accent_text: Color32,
    border: Color32,
}

fn state_style(state: &TorrentUiState) -> StateStyle {
    match state {
        TorrentUiState::Initializing => StateStyle {
            label: "Verificando",
            icon: Icon::Clock,
            pill_bg: colors::GRAY_SOFT,
            pill_fg: colors::SLATE,
            icon_bg: colors::GRAY_TINT,
            icon_fg: colors::SLATE_2,
            bar: colors::GRAY,
            accent_text: colors::SLATE_2,
            border: colors::BORDER,
        },
        TorrentUiState::Downloading => StateStyle {
            label: "Baixando",
            icon: Icon::ArrowDown,
            pill_bg: colors::TEAL_SOFT,
            pill_fg: colors::TEAL_DEEP,
            icon_bg: colors::TEAL_TINT,
            icon_fg: colors::TEAL_DARK,
            bar: colors::TEAL,
            accent_text: colors::TEAL_DARK,
            border: colors::BORDER,
        },
        TorrentUiState::Paused => StateStyle {
            label: "Pausado",
            icon: Icon::Pause,
            pill_bg: colors::GRAY_SOFT,
            pill_fg: colors::SLATE,
            icon_bg: colors::GRAY_TINT,
            icon_fg: colors::SLATE_2,
            bar: colors::GRAY,
            accent_text: colors::SLATE_2,
            border: colors::BORDER,
        },
        TorrentUiState::Finished => StateStyle {
            label: "Concluído",
            icon: Icon::Check,
            pill_bg: colors::GREEN_SOFT,
            pill_fg: colors::GREEN_FG,
            icon_bg: colors::GREEN_TINT,
            icon_fg: colors::GREEN_ICON,
            bar: colors::GREEN,
            accent_text: colors::GREEN_FG,
            border: colors::BORDER,
        },
        TorrentUiState::Error => StateStyle {
            label: "Erro",
            icon: Icon::Alert,
            pill_bg: colors::RED_SOFT,
            pill_fg: colors::RED_FG,
            icon_bg: colors::RED_TINT,
            icon_fg: colors::RED_FG,
            bar: colors::RED,
            accent_text: colors::RED_FG,
            border: colors::RED_BORDER,
        },
    }
}

fn pending_style() -> StateStyle {
    StateStyle {
        label: "Obtendo metadados",
        icon: Icon::Clock,
        pill_bg: colors::AMBER_SOFT,
        pill_fg: colors::AMBER_FG,
        icon_bg: colors::AMBER_SOFT,
        icon_fg: colors::AMBER_ICON,
        bar: colors::AMBER,
        accent_text: colors::AMBER_FG,
        border: colors::AMBER_BORDER,
    }
}

/// What a row asks the app to do after it is drawn.
enum RowAction {
    Pause(usize),
    Resume(usize),
    OpenFolder(String),
    AskRemove { id: usize, name: String },
}

struct RemoveDialog {
    id: usize,
    name: String,
}

pub struct LaunchTorrentApp {
    config: AppConfig,
    engine: EngineHandle,
    snapshot: EngineSnapshot,
    magnet_input: String,
    status_message: Option<String>,
    status_until: Option<Instant>,
    /// Sticky UI error (also mirrored from engine `last_error`).
    banner_error: Option<String>,
    /// Engine error the user closed; hidden until a different error arrives.
    dismissed_error: Option<String>,
    displayed_progress: std::collections::HashMap<usize, f32>,
    focus_magnet_once: bool,
    dir_warning: Option<String>,
    filter: Filter,
    remove_dialog: Option<RemoveDialog>,
}

impl LaunchTorrentApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);

        let config = AppConfig::load();
        let dir_warning = match config.ensure_download_dir() {
            Ok(()) => None,
            Err(e) => Some(format!("Pasta de download: {e:#}")),
        };
        let engine = spawn_engine(config.download_dir.clone());

        Self {
            config,
            engine,
            snapshot: EngineSnapshot::default(),
            magnet_input: String::new(),
            status_message: None,
            status_until: None,
            banner_error: dir_warning.clone(),
            dismissed_error: None,
            displayed_progress: std::collections::HashMap::new(),
            focus_magnet_once: true,
            dir_warning,
            filter: Filter::All,
            remove_dialog: None,
        }
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
        self.status_until = Some(Instant::now() + std::time::Duration::from_secs(5));
    }

    fn flash_error(&mut self, msg: impl Into<String>) {
        self.banner_error = Some(msg.into());
        self.dismissed_error = None;
        self.status_message = None;
        self.status_until = None;
    }

    fn poll_engine(&mut self) {
        let borrowed = self.engine.snapshot_rx.borrow().clone();
        if let Some(err) = borrowed.last_error.clone() {
            if self.dismissed_error.as_deref() != Some(err.as_str()) {
                self.banner_error = Some(err);
            }
        } else if let Some(w) = &self.dir_warning {
            if self.dismissed_error.as_deref() != Some(w.as_str()) {
                self.banner_error = Some(w.clone());
            }
        } else if self.banner_error.as_ref().is_some_and(|e| {
            e.starts_with("Magnet:")
                || e.starts_with("Arquivo .torrent:")
                || e.starts_with("Motor BitTorrent")
                || e.starts_with("Falha ao iniciar")
        }) {
            // Engine cleared last_error after a successful add.
            self.banner_error = None;
        }
        if borrowed.last_error.is_none() && self.dir_warning.is_none() {
            self.dismissed_error = None;
        }
        self.snapshot = borrowed;
    }

    fn dismiss_banner(&mut self) {
        self.dismissed_error = self.banner_error.take();
    }

    /// Fresh `arboard` instance per read — more reliable on Windows than a long-lived handle.
    fn read_clipboard_text() -> Option<String> {
        let mut clipboard = arboard::Clipboard::new().ok()?;
        let text = clipboard.get_text().ok()?;
        let text = text.replace('\r', "").trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    fn paste_magnet_from_clipboard(&mut self) -> bool {
        match Self::read_clipboard_text() {
            Some(text) => {
                self.magnet_input = text;
                self.flash("Magnet colado da área de transferência.");
                true
            }
            None => {
                self.flash("Nada para colar (área de transferência vazia).");
                false
            }
        }
    }

    /// Take Paste events so we own magnet paste (and avoid relying only on TextEdit focus).
    fn apply_paste_events(&mut self, ctx: &egui::Context) {
        let pasted = ctx.input_mut(|i| {
            let mut found = None;
            i.events.retain(|e| match e {
                egui::Event::Paste(s) => {
                    if found.is_none() {
                        let cleaned = s.replace('\r', "").trim().to_string();
                        if !cleaned.is_empty() {
                            found = Some(cleaned);
                        }
                    }
                    false
                }
                _ => true,
            });
            found
        });
        if let Some(text) = pasted {
            self.magnet_input = text;
        }
    }

    /// `.torrent` files dropped anywhere on the window are added directly.
    fn apply_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        let mut added = 0usize;
        let mut ignored = 0usize;
        for path in dropped {
            let is_torrent = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("torrent"));
            if is_torrent {
                self.engine.send(EngineCommand::AddTorrentFile(path));
                added += 1;
            } else {
                ignored += 1;
            }
        }
        if added > 0 {
            self.banner_error = None;
            let plural = if added == 1 { "" } else { "s" };
            self.flash(format!(
                "{added} arquivo{plural} .torrent adicionado{plural}."
            ));
        } else if ignored > 0 {
            self.flash_error("Só arquivos .torrent podem ser soltos na janela.");
        }
    }

    fn add_magnet(&mut self) {
        let url = self.magnet_input.trim().to_string();
        if url.is_empty() {
            self.flash_error("Cole um link magnet: antes de adicionar.");
            return;
        }
        let looks_magnet = url.starts_with("magnet:");
        let looks_hash =
            (url.len() == 40 || url.len() == 32) && url.chars().all(|c| c.is_ascii_hexdigit());
        if !(looks_magnet || looks_hash) {
            self.flash_error("Informe um magnet: ou info-hash (32/40 hex) válido.");
            return;
        }
        if !self.snapshot.ready && self.snapshot.last_error.is_some() {
            self.flash_error(
                self.snapshot
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Motor BitTorrent não está pronto.".into()),
            );
            return;
        }
        self.engine.send(EngineCommand::AddMagnet(url));
        self.magnet_input.clear();
        self.banner_error = None;
        self.flash("Resolvendo metadados do magnet… (pode levar até ~90s)");
    }

    fn open_torrent_file(&mut self) {
        let files = rfd::FileDialog::new()
            .add_filter("Torrent", &["torrent"])
            .set_title("Abrir arquivo .torrent")
            .pick_files();
        if let Some(paths) = files {
            let n = paths.len();
            for path in paths {
                self.engine.send(EngineCommand::AddTorrentFile(path));
            }
            if n > 0 {
                self.banner_error = None;
                self.flash(if n == 1 {
                    "Arquivo .torrent enviado para a sessão.".to_string()
                } else {
                    format!("{n} arquivos .torrent enviados para a sessão.")
                });
            }
        }
    }

    fn choose_download_dir(&mut self) {
        let dir = rfd::FileDialog::new()
            .set_title("Pasta de download")
            .set_directory(&self.config.download_dir)
            .pick_folder();
        if let Some(dir) = dir {
            let dir = config::normalize_download_dir(&dir);
            self.config.download_dir = dir.clone();
            if let Err(e) = self.config.save() {
                self.flash_error(format!("Não foi possível salvar config: {e:#}"));
                return;
            }
            if let Err(e) = self.config.ensure_download_dir() {
                self.dir_warning = Some(format!("Pasta de download: {e:#}"));
                self.flash_error(format!("Pasta de download: {e:#}"));
                return;
            }
            self.dir_warning = None;
            self.banner_error = None;
            self.engine.send(EngineCommand::SetDownloadDir(dir));
            self.flash("Pasta de download atualizada.");
        }
    }

    /// Opens Explorer on the download folder, selecting the torrent's output when it is there.
    fn open_in_explorer(&mut self, torrent_name: &str) {
        let dir = self.config.download_dir.clone();
        // Torrent names come from remote metadata: only use plain single-segment names.
        let safe_name = !torrent_name.is_empty()
            && !torrent_name.contains(['/', '\\', ':'])
            && torrent_name != "."
            && torrent_name != "..";
        let target = dir.join(torrent_name);
        let mut cmd = std::process::Command::new("explorer");
        if safe_name && target.exists() {
            cmd.arg("/select,").arg(&target);
        } else {
            cmd.arg(&dir);
        }
        if let Err(e) = cmd.spawn() {
            self.flash_error(format!("Não foi possível abrir a pasta: {e}"));
        }
    }

    fn smooth_progress(&mut self, id: usize, target: f32, dt: f32) -> f32 {
        let current = self.displayed_progress.entry(id).or_insert(target);
        let alpha = (dt * 8.0).clamp(0.0, 1.0);
        *current += (target - *current) * alpha;
        *current
    }

    fn count_for(&self, filter: Filter) -> usize {
        let pending = if filter.matches_pending() {
            self.snapshot.pending.len()
        } else {
            0
        };
        pending
            + self
                .snapshot
                .torrents
                .iter()
                .filter(|t| filter.matches_torrent(&t.state))
                .count()
    }

    // -----------------------------------------------------------------------
    // Panels
    // -----------------------------------------------------------------------

    fn sidebar(&mut self, ctx: &egui::Context) {
        let mut want_change_dir = false;
        egui::SidePanel::left("sidebar")
            .exact_width(SIDEBAR_WIDTH)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(colors::SIDEBAR)
                    .inner_margin(Margin::symmetric(16, 24)),
            )
            .show(ctx, |ui| {
                // Brand
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(10), colors::TEAL_DARK);
                    theme::paint_icon(
                        ui.painter(),
                        egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)),
                        Icon::Logo,
                        Color32::WHITE,
                    );
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new("LaunchTorrent")
                                .font(theme::semibold(16.0))
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .size(12.0)
                                .color(colors::SIDEBAR_MUTED),
                        );
                    });
                });

                ui.add_space(28.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("DOWNLOADS")
                            .font(theme::semibold(11.0))
                            .color(colors::SIDEBAR_MUTED),
                    );
                });
                ui.add_space(2.0);

                for filter in Filter::ALL {
                    let count = self.count_for(filter);
                    if filter_item(ui, filter, count, self.filter == filter).clicked() {
                        self.filter = filter;
                    }
                }

                // Destination folder card, pinned to the bottom.
                egui::TopBottomPanel::bottom("sidebar_folder")
                    .show_separator_line(false)
                    .frame(egui::Frame::NONE)
                    .show_inside(ui, |ui| {
                        egui::Frame::new()
                            .fill(colors::SIDEBAR_CARD)
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(Margin::same(14))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("PASTA DE DESTINO")
                                            .font(theme::semibold(11.0))
                                            .color(colors::SIDEBAR_MUTED),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(config::path_display(
                                                &self.config.download_dir,
                                            ))
                                            .monospace()
                                            .size(12.0)
                                            .color(colors::SIDEBAR_BRIGHT),
                                        )
                                        .wrap(),
                                    );
                                    ui.add_space(2.0);
                                    if theme::button(
                                        ui,
                                        ButtonKind::SidebarGhost,
                                        Some(Icon::Folder),
                                        "Alterar…",
                                        34.0,
                                    )
                                    .clicked()
                                    {
                                        want_change_dir = true;
                                    }
                                });
                            });
                    });
            });
        if want_change_dir {
            self.choose_download_dir();
        }
    }

    fn add_bar(&mut self, ctx: &egui::Context) {
        let mut want_paste = false;
        let mut want_add = false;
        let mut want_open_torrent = false;

        egui::TopBottomPanel::top("add_bar")
            .show_separator_line(true)
            .frame(
                egui::Frame::new()
                    .fill(colors::SURFACE)
                    .inner_margin(Margin {
                        left: 32,
                        right: 32,
                        top: 22,
                        bottom: 18,
                    }),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Adicionar torrent")
                        .font(theme::semibold(13.0))
                        .color(colors::TEXT),
                );
                ui.add_space(2.0);

                // Right-to-left: buttons claim their real width first, the field takes the rest.
                // Fixed-height row: a bare right_to_left layout would grab the panel's full height.
                let row_w = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(row_w, 48.0),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let magnet_id = egui::Id::new(MAGNET_EDIT_ID);
                    let focused = ui.ctx().memory(|m| m.has_focus(magnet_id));

                    if theme::button(
                        ui,
                        ButtonKind::Secondary,
                        Some(Icon::File),
                        "Abrir .torrent",
                        44.0,
                    )
                    .clicked()
                    {
                        want_open_torrent = true;
                    }
                    let (div, _) = ui.allocate_exact_size(egui::vec2(1.0, 32.0), egui::Sense::hover());
                    ui.painter().rect_filled(div, 0.0, colors::BORDER);
                    if theme::button(ui, ButtonKind::Primary, Some(Icon::Plus), "Adicionar", 44.0)
                        .clicked()
                    {
                        want_add = true;
                    }
                    if theme::button(ui, ButtonKind::Secondary, Some(Icon::Clipboard), "Colar", 44.0)
                        .clicked()
                    {
                        want_paste = true;
                    }

                    // Frame = 12px inner margin each side + 1px stroke each side.
                    let field_inner_w = (ui.available_width() - 26.0).max(160.0);
                    let field = egui::Frame::new()
                        .fill(colors::SURFACE)
                        .stroke(Stroke::new(
                            if focused { 1.5_f32 } else { 1.0 },
                            if focused { colors::TEAL_DARK } else { colors::BORDER_STRONG },
                        ))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(12, 0))
                        .show(ui, |ui| {
                            ui.set_width(field_inner_w);
                            ui.set_height(42.0);
                            // Explicit left-to-right: the parent row runs right-to-left.
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                theme::icon(ui, Icon::Link, 18.0, colors::TEXT_MUTED);
                                let edit_w = (ui.available_width() - 56.0).max(60.0);
                                let edit = ui.add(
                                    egui::TextEdit::singleline(&mut self.magnet_input)
                                        .id(magnet_id)
                                        .desired_width(edit_w)
                                        .font(egui::FontId::monospace(14.0))
                                        .text_color(colors::TEXT)
                                        .hint_text(
                                            RichText::new(
                                                "Cole um link magnet: ou info-hash (40 hex)",
                                            )
                                            .color(colors::TEXT_MUTED),
                                        )
                                        .frame(false),
                                );
                                if self.focus_magnet_once {
                                    edit.request_focus();
                                    self.focus_magnet_once = false;
                                }
                                if edit.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    want_add = true;
                                }
                                edit.context_menu(|ui| {
                                    if ui.button("Colar").clicked() {
                                        want_paste = true;
                                        ui.close_menu();
                                    }
                                    if ui.button("Limpar").clicked() {
                                        self.magnet_input.clear();
                                        ui.close_menu();
                                    }
                                });
                                kbd_hint(ui, "Enter");
                            });
                        });
                    if focused {
                        // Soft focus ring like the design.
                        ui.painter().rect_stroke(
                            field.response.rect.expand(2.0),
                            CornerRadius::same(12),
                            Stroke::new(3.0_f32, colors::TEAL.linear_multiply(0.18)),
                            StrokeKind::Outside,
                        );
                    }
                    },
                );

                ui.add_space(4.0);
                match &self.status_message {
                    Some(msg) => {
                        ui.label(
                            RichText::new(msg)
                                .font(theme::semibold(12.0))
                                .color(colors::TEAL_DARK),
                        );
                    }
                    None => {
                        ui.label(
                            RichText::new(
                                "Ctrl+V cola o magnet de qualquer lugar da janela · arraste arquivos .torrent para cá",
                            )
                            .size(12.0)
                            .color(colors::TEXT_MUTED),
                        );
                    }
                }
            });

        if want_paste {
            self.paste_magnet_from_clipboard();
            ctx.memory_mut(|m| m.request_focus(egui::Id::new(MAGNET_EDIT_ID)));
        }
        if want_add {
            self.add_magnet();
            ctx.memory_mut(|m| m.request_focus(egui::Id::new(MAGNET_EDIT_ID)));
        }
        if want_open_torrent {
            self.open_torrent_file();
        }
    }

    fn status_bar(&self, ctx: &egui::Context) {
        let (dot, text, fg) = if self.snapshot.ready {
            (colors::GREEN, "Motor pronto", colors::GREEN_FG)
        } else if self.snapshot.last_error.is_some() {
            (colors::RED, "Motor com falha", colors::RED_FG)
        } else {
            (colors::AMBER, "Inicializando motor…", colors::AMBER_FG)
        };
        let (down, up, peers) = self.totals();
        let total = self.snapshot.torrents.len() + self.snapshot.pending.len();

        egui::TopBottomPanel::bottom("status_bar")
            .show_separator_line(true)
            .frame(
                egui::Frame::new()
                    .fill(colors::SURFACE)
                    .inner_margin(Margin::symmetric(32, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, dot);
                    ui.label(RichText::new(text).font(theme::semibold(12.0)).color(fg));
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(format!(
                            "{total} torrent{} · {peers} peers conectados",
                            if total == 1 { "" } else { "s" }
                        ))
                        .size(12.0)
                        .color(colors::TEXT_MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "↓ {} · ↑ {}",
                                format_rate(down),
                                format_rate(up)
                            ))
                            .size(12.0)
                            .color(colors::TEXT_MUTED),
                        );
                    });
                });
            });
    }

    fn totals(&self) -> (u64, u64, u32) {
        self.snapshot
            .torrents
            .iter()
            .fold((0, 0, 0), |(d, u, p), t| {
                (d + t.download_bps, u + t.upload_bps, p + t.peers)
            })
    }

    fn main_area(&mut self, ctx: &egui::Context) -> Vec<RowAction> {
        let dt = ctx.input(|i| i.stable_dt);
        let mut actions = Vec::new();
        let mut dismiss = false;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(colors::BG)
                    .inner_margin(Margin::symmetric(32, 20)),
            )
            .show(ctx, |ui| {
                if let Some(err) = self.banner_error.clone() {
                    dismiss = error_banner(ui, &err);
                    ui.add_space(14.0);
                }

                // Heading + aggregate rates
                let (down, up, _) = self.totals();
                let visible = self.count_for(self.filter);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.filter.label())
                            .font(theme::semibold(20.0))
                            .color(colors::TEXT),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{visible} {}",
                            if visible == 1 { "item" } else { "itens" }
                        ))
                        .size(14.0)
                        .color(colors::TEXT_MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        theme::rate_chip(ui, Icon::ArrowUp, &format_rate(up), colors::SLATE);
                        theme::rate_chip(
                            ui,
                            Icon::ArrowDown,
                            &format_rate(down),
                            colors::TEAL_DARK,
                        );
                    });
                });
                ui.add_space(10.0);

                if visible == 0 {
                    empty_state(ui, self.filter);
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 10.0;
                        if self.filter.matches_pending() {
                            for p in self.snapshot.pending.clone() {
                                draw_pending_row(ui, &p);
                            }
                        }
                        let torrents: Vec<TorrentSnapshot> = self
                            .snapshot
                            .torrents
                            .iter()
                            .filter(|t| self.filter.matches_torrent(&t.state))
                            .cloned()
                            .collect();
                        for t in &torrents {
                            let prog = self.smooth_progress(t.id, t.progress, dt);
                            if let Some(a) = draw_torrent_row(ui, t, prog) {
                                actions.push(a);
                            }
                        }
                        ui.add_space(8.0);
                    });
            });

        if dismiss {
            self.dismiss_banner();
        }
        actions
    }

    fn remove_modal(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &self.remove_dialog else {
            return;
        };
        let (id, name) = (dialog.id, dialog.name.clone());
        let mut choice: Option<Option<bool>> = None; // Some(None) = cancel

        let resp = egui::Modal::new(egui::Id::new("remove_dialog"))
            .backdrop_color(Color32::from_black_alpha(90))
            .frame(
                egui::Frame::new()
                    .fill(colors::SURFACE)
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::same(24))
                    .stroke(Stroke::new(1.0_f32, colors::BORDER)),
            )
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(
                    RichText::new("Remover torrent?")
                        .font(theme::semibold(18.0))
                        .color(colors::TEXT),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(RichText::new(&name).size(14.0).color(colors::SLATE)).wrap(),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Você pode só tirar da lista e manter os arquivos baixados, ou apagar tudo do disco.",
                    )
                    .size(13.0)
                    .color(colors::TEXT_MUTED),
                );
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if theme::button(ui, ButtonKind::Secondary, None, "Cancelar", 38.0).clicked() {
                        choice = Some(None);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::button(ui, ButtonKind::Danger, Some(Icon::Trash), "Apagar arquivos", 38.0)
                            .clicked()
                        {
                            choice = Some(Some(true));
                        }
                        if theme::button(ui, ButtonKind::Primary, None, "Só remover da lista", 38.0)
                            .clicked()
                        {
                            choice = Some(Some(false));
                        }
                    });
                });
            });

        if resp.should_close() && choice.is_none() {
            choice = Some(None);
        }
        match choice {
            Some(Some(delete_files)) => {
                self.engine.send(EngineCommand::Remove { id, delete_files });
                self.displayed_progress.remove(&id);
                self.flash(if delete_files {
                    "Torrent e arquivos removidos."
                } else {
                    "Torrent removido da lista (arquivos mantidos)."
                });
                self.remove_dialog = None;
            }
            Some(None) => self.remove_dialog = None,
            None => {}
        }
    }
}

impl eframe::App for LaunchTorrentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_engine();
        self.apply_paste_events(ctx);
        self.apply_dropped_files(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        if let Some(until) = self.status_until {
            if Instant::now() > until {
                self.status_message = None;
                self.status_until = None;
            }
        }

        // Panel order matters: sidebar first so it spans the full height.
        self.sidebar(ctx);
        self.status_bar(ctx);
        self.add_bar(ctx);
        let actions = self.main_area(ctx);

        for action in actions {
            match action {
                RowAction::Pause(id) => self.engine.send(EngineCommand::Pause(id)),
                RowAction::Resume(id) => self.engine.send(EngineCommand::Resume(id)),
                RowAction::OpenFolder(name) => self.open_in_explorer(&name),
                RowAction::AskRemove { id, name } => {
                    self.remove_dialog = Some(RemoveDialog { id, name })
                }
            }
        }

        self.remove_modal(ctx);
        drop_overlay(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.config.save();
        self.engine.send(EngineCommand::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// Widgets used by the panels
// ---------------------------------------------------------------------------

fn filter_item(ui: &mut egui::Ui, filter: Filter, count: usize, active: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::click());
    let painter = ui.painter();
    let bg = if active {
        colors::SIDEBAR_ACTIVE
    } else if resp.hovered() {
        colors::SIDEBAR_HOVER
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, CornerRadius::same(8), bg);

    let dot_c = egui::pos2(rect.min.x + 16.0, rect.center().y);
    painter.circle_filled(dot_c, 4.0, filter.dot());

    let text_color = if active {
        Color32::WHITE
    } else {
        colors::SIDEBAR_TEXT
    };
    let font = if active {
        theme::semibold(14.0)
    } else {
        theme::regular(14.0)
    };
    let label = painter.layout_no_wrap(filter.label().to_owned(), font, text_color);
    painter.galley(
        egui::pos2(rect.min.x + 30.0, rect.center().y - label.size().y * 0.5),
        label,
        text_color,
    );

    let (badge_bg, badge_fg) = if active {
        (colors::TEAL_DARK, Color32::WHITE)
    } else {
        (colors::SIDEBAR_BADGE, colors::SIDEBAR_TEXT)
    };
    let num = painter.layout_no_wrap(count.to_string(), theme::semibold(12.0), badge_fg);
    let bw = (num.size().x + 14.0).max(26.0);
    let badge = egui::Rect::from_center_size(
        egui::pos2(rect.max.x - 12.0 - bw * 0.5, rect.center().y),
        egui::vec2(bw, 20.0),
    );
    painter.rect_filled(badge, CornerRadius::same(10), badge_bg);
    painter.galley(badge.center() - num.size() * 0.5, num, badge_fg);

    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn kbd_hint(ui: &mut egui::Ui, text: &str) {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::regular(11.0), colors::TEXT_MUTED);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(galley.size().x + 12.0, 20.0),
        egui::Sense::hover(),
    );
    ui.painter().rect(
        rect,
        CornerRadius::same(5),
        Color32::TRANSPARENT,
        Stroke::new(1.0_f32, colors::BORDER),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        rect.center() - galley.size() * 0.5,
        galley,
        colors::TEXT_MUTED,
    );
}

/// Returns true when the user closes the banner.
fn error_banner(ui: &mut egui::Ui, err: &str) -> bool {
    let mut closed = false;
    egui::Frame::new()
        .fill(colors::RED_TINT)
        .stroke(Stroke::new(1.0_f32, colors::RED_BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                theme::icon(ui, Icon::Alert, 18.0, colors::RED_FG);
                let text_w = (ui.available_width() - 44.0).max(80.0);
                ui.add_sized(
                    [text_w, 20.0],
                    egui::Label::new(RichText::new(err).size(13.0).color(colors::RED_FG)).wrap(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::icon_button(ui, ButtonKind::Danger, Icon::Close, "Fechar aviso", 30.0)
                        .clicked()
                    {
                        closed = true;
                    }
                });
            });
        });
    closed
}

fn empty_state(ui: &mut egui::Ui, filter: Filter) {
    let (title, body) = if filter == Filter::All {
        (
            "Nenhum download ainda".to_string(),
            "Cole um link magnet acima, abra um .torrent ou arraste o arquivo para a janela.",
        )
    } else {
        (
            format!("Nada em “{}”", filter.label()),
            "Os torrents aparecem aqui conforme mudam de estado.",
        )
    };
    ui.add_space(24.0);
    egui::Frame::new()
        .fill(colors::SURFACE)
        .stroke(Stroke::new(1.0_f32, colors::BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(24, 40))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(12), colors::TEAL_TINT);
                theme::paint_icon(
                    ui.painter(),
                    egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0)),
                    Icon::Logo,
                    colors::TEAL_DARK,
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(title)
                        .font(theme::semibold(16.0))
                        .color(colors::TEXT),
                );
                ui.label(RichText::new(body).size(13.0).color(colors::TEXT_MUTED));
            });
        });
}

/// Full-window hint while files are dragged over the app.
fn drop_overlay(ctx: &egui::Context) {
    let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
    if !hovering {
        return;
    }
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("drop_overlay"),
    ));
    painter.rect_filled(
        screen,
        0.0,
        Color32::from_rgba_unmultiplied(19, 34, 49, 170),
    );
    let card = egui::Rect::from_center_size(screen.center(), egui::vec2(360.0, 160.0));
    painter.rect(
        card,
        CornerRadius::same(16),
        colors::SURFACE,
        Stroke::new(2.0_f32, colors::TEAL),
        StrokeKind::Inside,
    );
    theme::paint_icon(
        &painter,
        egui::Rect::from_center_size(
            card.center() - egui::vec2(0.0, 26.0),
            egui::vec2(32.0, 32.0),
        ),
        Icon::Logo,
        colors::TEAL_DARK,
    );
    painter.text(
        card.center() + egui::vec2(0.0, 18.0),
        egui::Align2::CENTER_CENTER,
        "Solte para adicionar",
        theme::semibold(16.0),
        colors::TEXT,
    );
    painter.text(
        card.center() + egui::vec2(0.0, 42.0),
        egui::Align2::CENTER_CENTER,
        "Somente arquivos .torrent",
        theme::regular(12.0),
        colors::TEXT_MUTED,
    );
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

const ROW_ICON: f32 = 40.0;
const ROW_SPEEDS_W: f32 = 104.0;
const ROW_BTN: f32 = 36.0;
const ROW_BTNS_W: f32 = ROW_BTN * 3.0 + 6.0 * 2.0;
const ROW_GAP: f32 = 16.0;

fn row_frame(border: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(colors::SURFACE)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(18, 14))
}

fn state_icon_box(ui: &mut egui::Ui, style: &StateStyle) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ROW_ICON, ROW_ICON), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(10), style.icon_bg);
    theme::paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)),
        style.icon,
        style.icon_fg,
    );
}

/// Name (truncated, full text on hover) followed by the state pill.
fn name_with_pill(ui: &mut egui::Ui, name: &str, style: &StateStyle, width: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let pill_w = theme::pill_width(ui, style.label);
        let name_galley_w = ui
            .painter()
            .layout_no_wrap(name.to_owned(), theme::semibold(15.0), colors::TEXT)
            .size()
            .x;
        let name_w = name_galley_w.min((width - pill_w - 10.0).max(40.0));
        ui.add_sized(
            [name_w, 22.0],
            egui::Label::new(
                RichText::new(name)
                    .font(theme::semibold(15.0))
                    .color(colors::TEXT),
            )
            .truncate(),
        );
        theme::pill(ui, style.label, style.pill_bg, style.pill_fg);
    });
}

fn meta_line(ui: &mut egui::Ui, accent: &str, accent_color: Color32, rest: &[String]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        ui.label(
            RichText::new(accent)
                .font(theme::semibold(13.0))
                .color(accent_color),
        );
        for part in rest.iter().filter(|s| !s.is_empty()) {
            ui.label(RichText::new(part).size(13.0).color(colors::TEXT_MUTED));
        }
    });
}

fn speeds_column(ui: &mut egui::Ui, down: Option<u64>, up: Option<u64>) {
    ui.allocate_ui_with_layout(
        egui::vec2(ROW_SPEEDS_W, ROW_ICON),
        Layout::top_down(Align::Min).with_main_align(Align::Center),
        |ui| {
            ui.set_width(ROW_SPEEDS_W);
            ui.spacing_mut().item_spacing.y = 2.0;
            let fmt = |v: Option<u64>| v.map(format_rate).unwrap_or_else(|| "—".into());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                theme::icon(ui, Icon::ArrowDown, 13.0, colors::TEAL_DARK);
                ui.label(
                    RichText::new(fmt(down))
                        .font(theme::semibold(13.0))
                        .color(colors::TEAL_DARK),
                );
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                theme::icon(ui, Icon::ArrowUp, 13.0, colors::SLATE_2);
                ui.label(RichText::new(fmt(up)).size(13.0).color(colors::SLATE_2));
            });
        },
    );
}

fn middle_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() - ROW_ICON - ROW_SPEEDS_W - ROW_BTNS_W - ROW_GAP * 3.0).max(160.0)
}

fn draw_pending_row(ui: &mut egui::Ui, p: &PendingAdd) {
    let style = pending_style();
    row_frame(style.border).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = ROW_GAP;
            state_icon_box(ui, &style);
            let mid_w = middle_width(ui);
            ui.allocate_ui_with_layout(
                egui::vec2(mid_w, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(mid_w);
                    ui.spacing_mut().item_spacing.y = 8.0;
                    name_with_pill(ui, &p.label, &style, mid_w);
                    theme::progress_bar(ui, mid_w, None, style.bar);
                    let elapsed = p.started_at.elapsed().as_secs();
                    let (kind, what, timing) = match p.kind {
                        PendingKind::Magnet => (
                            "magnet",
                            "Consultando DHT e trackers…".to_string(),
                            format!("{elapsed} s de até {} s", MAGNET_RESOLVE_TIMEOUT.as_secs()),
                        ),
                        PendingKind::TorrentFile => (
                            ".torrent",
                            "Lendo arquivo e conectando…".to_string(),
                            format!("{elapsed} s"),
                        ),
                    };
                    meta_line(ui, kind, style.accent_text, &[what, timing]);
                },
            );
            speeds_column(ui, None, None);
            // Keep columns aligned with full rows (no actions while resolving).
            ui.add_space(ROW_BTNS_W);
        });
    });
}

fn draw_torrent_row(ui: &mut egui::Ui, t: &TorrentSnapshot, shown: f32) -> Option<RowAction> {
    let style = state_style(&t.state);
    let mut action = None;
    row_frame(style.border).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = ROW_GAP;
            state_icon_box(ui, &style);
            let mid_w = middle_width(ui);
            ui.allocate_ui_with_layout(
                egui::vec2(mid_w, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(mid_w);
                    ui.spacing_mut().item_spacing.y = 8.0;
                    name_with_pill(ui, &t.name, &style, mid_w);
                    theme::progress_bar(ui, mid_w, Some(shown), style.bar);
                    let pct = format_pct(shown);
                    meta_line(
                        ui,
                        &pct,
                        style.accent_text,
                        &[sizes_text(t), detail_text(t)],
                    );
                    if let Some(err) = &t.error {
                        ui.add(
                            egui::Label::new(RichText::new(err).size(13.0).color(colors::RED_FG))
                                .wrap(),
                        );
                    }
                },
            );
            speeds_column(ui, Some(t.download_bps), Some(t.upload_bps));

            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                match t.state {
                    TorrentUiState::Paused | TorrentUiState::Error => {
                        if theme::icon_button(
                            ui,
                            ButtonKind::Primary,
                            Icon::Play,
                            "Retomar",
                            ROW_BTN,
                        )
                        .clicked()
                        {
                            action = Some(RowAction::Resume(t.id));
                        }
                    }
                    TorrentUiState::Downloading
                    | TorrentUiState::Initializing
                    | TorrentUiState::Finished => {
                        if theme::icon_button(
                            ui,
                            ButtonKind::Secondary,
                            Icon::Pause,
                            "Pausar",
                            ROW_BTN,
                        )
                        .clicked()
                        {
                            action = Some(RowAction::Pause(t.id));
                        }
                    }
                }
                if theme::icon_button(
                    ui,
                    ButtonKind::Secondary,
                    Icon::Folder,
                    "Abrir pasta",
                    ROW_BTN,
                )
                .clicked()
                {
                    action = Some(RowAction::OpenFolder(t.name.clone()));
                }
                if theme::icon_button(ui, ButtonKind::Danger, Icon::Trash, "Remover…", ROW_BTN)
                    .clicked()
                {
                    action = Some(RowAction::AskRemove {
                        id: t.id,
                        name: t.name.clone(),
                    });
                }
            });
        });
    });
    action
}

fn sizes_text(t: &TorrentSnapshot) -> String {
    if t.total_bytes == 0 {
        "tamanho desconhecido".into()
    } else if t.state == TorrentUiState::Finished {
        format_bytes(t.total_bytes)
    } else {
        format!(
            "{} de {}",
            format_bytes(t.progress_bytes),
            format_bytes(t.total_bytes)
        )
    }
}

fn detail_text(t: &TorrentSnapshot) -> String {
    let peers = format!("{} peers", t.peers);
    match t.state {
        TorrentUiState::Initializing => "verificando arquivos…".into(),
        TorrentUiState::Paused => "pausado".into(),
        TorrentUiState::Error => peers,
        TorrentUiState::Finished => {
            if t.upload_bps > 0 {
                format!("{peers} · semeando")
            } else {
                peers
            }
        }
        TorrentUiState::Downloading => {
            let remaining = t.total_bytes.saturating_sub(t.progress_bytes);
            if t.download_bps > 0 && remaining > 0 {
                format!(
                    "{peers} · faltam {}",
                    format_eta(remaining / t.download_bps)
                )
            } else {
                format!("{peers} · aguardando peers")
            }
        }
    }
}

fn format_pct(fraction: f32) -> String {
    format!("{:.1}%", fraction.clamp(0.0, 1.0) * 100.0).replace('.', ",")
}

fn format_eta(secs: u64) -> String {
    if secs < 60 {
        format!("~{secs} s")
    } else if secs < 3600 {
        format!("~{} min", (secs + 30) / 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("~{h} h {m:02} min")
    }
}

fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let v = n as f64;
    let s = if v >= GB {
        format!("{:.2} GiB", v / GB)
    } else if v >= MB {
        format!("{:.1} MiB", v / MB)
    } else if v >= KB {
        format!("{:.0} KiB", v / KB)
    } else {
        format!("{n} B")
    };
    s.replace('.', ",")
}

fn format_rate(bps: u64) -> String {
    format!("{}/s", format_bytes(bps))
}
