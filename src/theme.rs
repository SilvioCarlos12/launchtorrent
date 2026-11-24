//! Visual tokens, fonts and hand-painted widgets for the LaunchTorrent UI.
//!
//! The look follows the "LaunchTorrent Redesign" canvas: a dark slate sidebar,
//! a light workspace, teal as the single action color and one soft tint per
//! torrent state.

use egui::{
    Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect,
    Response, Sense, Shape, Stroke, StrokeKind, Style, Ui, Visuals,
};
use std::sync::Arc;

pub mod colors {
    use egui::Color32;

    // Workspace
    pub const BG: Color32 = Color32::from_rgb(0xF4, 0xF7, 0xFA);
    pub const SURFACE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);
    pub const BORDER: Color32 = Color32::from_rgb(0xDC, 0xE3, 0xEA);
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(0xC5, 0xD0, 0xDC);
    pub const TRACK: Color32 = Color32::from_rgb(0xE6, 0xEC, 0xF1);
    pub const TEXT: Color32 = Color32::from_rgb(0x14, 0x22, 0x33);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x5B, 0x6B, 0x7D);
    pub const SLATE: Color32 = Color32::from_rgb(0x33, 0x41, 0x55);
    pub const SLATE_2: Color32 = Color32::from_rgb(0x47, 0x55, 0x69);

    // Sidebar
    pub const SIDEBAR: Color32 = Color32::from_rgb(0x13, 0x22, 0x31);
    pub const SIDEBAR_TEXT: Color32 = Color32::from_rgb(0xC9, 0xD4, 0xE0);
    pub const SIDEBAR_MUTED: Color32 = Color32::from_rgb(0x93, 0xA4, 0xB6);
    pub const SIDEBAR_ACTIVE: Color32 = Color32::from_rgb(0x1E, 0x3A, 0x4E);
    pub const SIDEBAR_HOVER: Color32 = Color32::from_rgb(0x1A, 0x2E, 0x40);
    pub const SIDEBAR_CARD: Color32 = Color32::from_rgb(0x1B, 0x2E, 0x40);
    pub const SIDEBAR_BADGE: Color32 = Color32::from_rgb(0x22, 0x38, 0x4C);
    pub const SIDEBAR_BORDER: Color32 = Color32::from_rgb(0x36, 0x50, 0x6A);
    pub const SIDEBAR_BRIGHT: Color32 = Color32::from_rgb(0xE2, 0xE8, 0xF0);

    // Teal (action + downloading)
    pub const TEAL: Color32 = Color32::from_rgb(0x0D, 0x94, 0x88);
    pub const TEAL_DARK: Color32 = Color32::from_rgb(0x0F, 0x76, 0x6E);
    pub const TEAL_DEEP: Color32 = Color32::from_rgb(0x11, 0x5E, 0x59);
    pub const TEAL_SOFT: Color32 = Color32::from_rgb(0xCC, 0xFB, 0xF1);
    pub const TEAL_TINT: Color32 = Color32::from_rgb(0xE6, 0xFA, 0xF6);
    pub const TEAL_DOT: Color32 = Color32::from_rgb(0x2D, 0xD4, 0xBF);

    // Amber (resolving metadata)
    pub const AMBER: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
    pub const AMBER_SOFT: Color32 = Color32::from_rgb(0xFE, 0xF3, 0xC7);
    pub const AMBER_FG: Color32 = Color32::from_rgb(0x92, 0x40, 0x0E);
    pub const AMBER_ICON: Color32 = Color32::from_rgb(0xB4, 0x53, 0x09);
    pub const AMBER_BORDER: Color32 = Color32::from_rgb(0xF3, 0xDF, 0xA2);

    // Green (finished / seeding)
    pub const GREEN: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);
    pub const GREEN_SOFT: Color32 = Color32::from_rgb(0xDC, 0xFC, 0xE7);
    pub const GREEN_FG: Color32 = Color32::from_rgb(0x16, 0x65, 0x34);
    pub const GREEN_TINT: Color32 = Color32::from_rgb(0xE9, 0xFB, 0xEF);
    pub const GREEN_ICON: Color32 = Color32::from_rgb(0x15, 0x80, 0x3D);
    pub const GREEN_DOT: Color32 = Color32::from_rgb(0x4A, 0xDE, 0x80);

    // Gray (paused)
    pub const GRAY: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
    pub const GRAY_SOFT: Color32 = Color32::from_rgb(0xE2, 0xE8, 0xF0);
    pub const GRAY_TINT: Color32 = Color32::from_rgb(0xEE, 0xF2, 0xF6);

    // Red (error / destructive)
    pub const RED: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);
    pub const RED_SOFT: Color32 = Color32::from_rgb(0xFE, 0xE2, 0xE2);
    pub const RED_FG: Color32 = Color32::from_rgb(0xB9, 0x1C, 0x1C);
    pub const RED_TINT: Color32 = Color32::from_rgb(0xFE, 0xF0, 0xF0);
    pub const RED_BORDER: Color32 = Color32::from_rgb(0xF5, 0xC2, 0xC2);
    pub const RED_BUTTON: Color32 = Color32::from_rgb(0xFF, 0xF5, 0xF5);
    pub const RED_DOT: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71);
}

const SEMIBOLD: &str = "semibold";

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);

    let mut style = Style::default();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style.visuals = light_visuals();
    ctx.set_style(style);
}

/// Prefer Segoe UI / Consolas on Windows; fall back to egui's bundled fonts.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let default_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let font_dir = std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_default();
    let mut load = |key: &str, file: &str| -> bool {
        match std::fs::read(font_dir.join(file)) {
            Ok(bytes) => {
                fonts
                    .font_data
                    .insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
                true
            }
            Err(_) => false,
        }
    };

    let has_regular = load("segoe-ui", "segoeui.ttf");
    let has_semibold = load("segoe-ui-semibold", "seguisb.ttf");
    let has_mono = load("consolas", "consola.ttf");

    if has_regular {
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe-ui".to_owned());
        }
    }
    if has_mono {
        if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, "consolas".to_owned());
        }
    }

    // The semibold family must always exist, even if only as a fallback.
    let mut semibold = default_prop;
    if has_regular {
        semibold.insert(0, "segoe-ui".to_owned());
    }
    if has_semibold {
        semibold.insert(0, "segoe-ui-semibold".to_owned());
    }
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), semibold);

    ctx.set_fonts(fonts);
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

fn light_visuals() -> Visuals {
    let mut v = Visuals::light();
    v.override_text_color = Some(colors::TEXT);
    v.panel_fill = colors::BG;
    v.window_fill = colors::SURFACE;
    v.window_stroke = Stroke::new(1.0_f32, colors::BORDER);
    v.extreme_bg_color = colors::SURFACE;
    v.faint_bg_color = colors::BG;
    v.widgets.inactive.bg_fill = colors::SURFACE;
    v.widgets.inactive.weak_bg_fill = colors::SURFACE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, colors::TEXT);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    v.widgets.hovered.bg_fill = colors::SURFACE_HOVER;
    v.widgets.hovered.weak_bg_fill = colors::SURFACE_HOVER;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, colors::TEXT);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, colors::BORDER_STRONG);
    v.widgets.active.bg_fill = colors::TEAL_DARK;
    v.widgets.active.weak_bg_fill = colors::TEAL_DARK;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, colors::TEAL_DEEP);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    v.selection.bg_fill = colors::TEAL.linear_multiply(0.25);
    v.selection.stroke = Stroke::new(1.5_f32, colors::TEAL_DARK);
    v.text_cursor.stroke = Stroke::new(2.0_f32, colors::TEAL_DARK);
    v.hyperlink_color = colors::TEAL_DARK;
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    v
}

// ---------------------------------------------------------------------------
// Icons — painted with strokes so they stay crisp at any DPI.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Logo,
    Pause,
    Play,
    Folder,
    Trash,
    ArrowDown,
    ArrowUp,
    Check,
    Clock,
    Alert,
    Plus,
    Clipboard,
    File,
    Link,
    Close,
}

pub fn paint_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let w = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), egui::vec2(w, w));
    let p = |x: f32, y: f32| Pos2::new(r.min.x + x * w, r.min.y + y * w);
    let s = Stroke::new((w * 0.1).clamp(1.4, 2.4), color);
    let line = |pts: Vec<Pos2>| painter.add(Shape::line(pts, s));

    match icon {
        Icon::Logo => {
            line(vec![p(0.5, 0.14), p(0.5, 0.62)]);
            line(vec![p(0.26, 0.40), p(0.5, 0.64), p(0.74, 0.40)]);
            line(vec![p(0.18, 0.84), p(0.82, 0.84)]);
        }
        Icon::Pause => {
            for x in [0.32, 0.56] {
                painter.rect_filled(
                    Rect::from_min_max(p(x, 0.22), p(x + 0.13, 0.78)),
                    CornerRadius::same(1),
                    color,
                );
            }
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![p(0.3, 0.2), p(0.8, 0.5), p(0.3, 0.8)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Folder => {
            line(vec![
                p(0.12, 0.32),
                p(0.12, 0.22),
                p(0.40, 0.22),
                p(0.48, 0.32),
                p(0.88, 0.32),
                p(0.88, 0.80),
                p(0.12, 0.80),
                p(0.12, 0.32),
            ]);
        }
        Icon::Trash => {
            line(vec![p(0.16, 0.28), p(0.84, 0.28)]);
            line(vec![
                p(0.38, 0.28),
                p(0.38, 0.16),
                p(0.62, 0.16),
                p(0.62, 0.28),
            ]);
            line(vec![
                p(0.26, 0.28),
                p(0.30, 0.86),
                p(0.70, 0.86),
                p(0.74, 0.28),
            ]);
            line(vec![p(0.44, 0.42), p(0.44, 0.72)]);
            line(vec![p(0.56, 0.42), p(0.56, 0.72)]);
        }
        Icon::ArrowDown => {
            line(vec![p(0.5, 0.16), p(0.5, 0.82)]);
            line(vec![p(0.24, 0.56), p(0.5, 0.82), p(0.76, 0.56)]);
        }
        Icon::ArrowUp => {
            line(vec![p(0.5, 0.84), p(0.5, 0.18)]);
            line(vec![p(0.24, 0.44), p(0.5, 0.18), p(0.76, 0.44)]);
        }
        Icon::Check => {
            line(vec![p(0.2, 0.52), p(0.42, 0.74), p(0.82, 0.28)]);
        }
        Icon::Clock => {
            painter.circle_stroke(r.center(), w * 0.38, s);
            line(vec![p(0.5, 0.28), p(0.5, 0.5), p(0.66, 0.6)]);
        }
        Icon::Alert => {
            painter.circle_stroke(r.center(), w * 0.38, s);
            line(vec![p(0.5, 0.30), p(0.5, 0.56)]);
            painter.circle_filled(p(0.5, 0.70), w * 0.055, color);
        }
        Icon::Plus => {
            line(vec![p(0.5, 0.18), p(0.5, 0.82)]);
            line(vec![p(0.18, 0.5), p(0.82, 0.5)]);
        }
        Icon::Clipboard => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.2, 0.2), p(0.8, 0.88)),
                CornerRadius::same(2),
                s,
                StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.36, 0.12), p(0.64, 0.28)),
                CornerRadius::same(1),
                s,
                StrokeKind::Middle,
            );
        }
        Icon::File => {
            line(vec![
                p(0.58, 0.12),
                p(0.24, 0.12),
                p(0.24, 0.88),
                p(0.76, 0.88),
                p(0.76, 0.30),
                p(0.58, 0.12),
            ]);
            line(vec![p(0.58, 0.12), p(0.58, 0.30), p(0.76, 0.30)]);
        }
        Icon::Link => {
            let rad = CornerRadius::same((w * 0.14) as u8);
            painter.rect_stroke(
                Rect::from_min_max(p(0.08, 0.36), p(0.58, 0.64)),
                rad,
                s,
                StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.42, 0.36), p(0.92, 0.64)),
                rad,
                s,
                StrokeKind::Middle,
            );
        }
        Icon::Close => {
            line(vec![p(0.26, 0.26), p(0.74, 0.74)]);
            line(vec![p(0.74, 0.26), p(0.26, 0.74)]);
        }
    }
}

/// Allocate a square and paint an icon in it (non-interactive).
pub fn icon(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
    paint_icon(ui.painter(), rect, icon, color);
    resp
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Danger,
    /// Outline button on the dark sidebar.
    SidebarGhost,
}

struct ButtonColors {
    fill: Color32,
    fill_hover: Color32,
    stroke: Color32,
    text: Color32,
}

fn button_colors(kind: ButtonKind) -> ButtonColors {
    match kind {
        ButtonKind::Primary => ButtonColors {
            fill: colors::TEAL_DARK,
            fill_hover: colors::TEAL_DEEP,
            stroke: Color32::TRANSPARENT,
            text: Color32::WHITE,
        },
        ButtonKind::Secondary => ButtonColors {
            fill: colors::SURFACE,
            fill_hover: colors::SURFACE_HOVER,
            stroke: colors::BORDER,
            text: colors::SLATE,
        },
        ButtonKind::Danger => ButtonColors {
            fill: colors::RED_BUTTON,
            fill_hover: colors::RED_SOFT,
            stroke: colors::RED_BORDER,
            text: colors::RED_FG,
        },
        ButtonKind::SidebarGhost => ButtonColors {
            fill: Color32::TRANSPARENT,
            fill_hover: colors::SIDEBAR_BADGE,
            stroke: colors::SIDEBAR_BORDER,
            text: colors::SIDEBAR_BRIGHT,
        },
    }
}

/// Button with an optional leading icon, painted to match the design.
pub fn button(
    ui: &mut Ui,
    kind: ButtonKind,
    icon: Option<Icon>,
    label: &str,
    height: f32,
) -> Response {
    let c = button_colors(kind);
    let font = if kind == ButtonKind::Primary {
        semibold(14.0)
    } else {
        regular(14.0)
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, c.text);
    let pad_x = if kind == ButtonKind::Primary {
        18.0
    } else {
        14.0
    };
    let icon_w = if icon.is_some() { 16.0 + 8.0 } else { 0.0 };
    let size = egui::vec2(pad_x * 2.0 + icon_w + galley.size().x, height);

    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered() || resp.has_focus();
        let fill = if hovered { c.fill_hover } else { c.fill };
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(8),
            fill,
            Stroke::new(1.0_f32, c.stroke),
            StrokeKind::Inside,
        );
        let mut x = rect.min.x + pad_x;
        if let Some(icon) = icon {
            let ir =
                Rect::from_center_size(Pos2::new(x + 8.0, rect.center().y), egui::vec2(16.0, 16.0));
            paint_icon(painter, ir, icon, c.text);
            x += icon_w;
        }
        let text_pos = Pos2::new(x, rect.center().y - galley.size().y * 0.5);
        painter.galley(text_pos, galley, c.text);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Square icon-only button; always carries a tooltip for discoverability.
pub fn icon_button(
    ui: &mut Ui,
    kind: ButtonKind,
    icon: Icon,
    tooltip: &str,
    size: f32,
) -> Response {
    let c = button_colors(kind);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered() || resp.has_focus();
        let fill = if hovered { c.fill_hover } else { c.fill };
        ui.painter().rect(
            rect,
            CornerRadius::same(8),
            fill,
            Stroke::new(1.0_f32, c.stroke),
            StrokeKind::Inside,
        );
        paint_icon(
            ui.painter(),
            Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)),
            icon,
            c.text,
        );
    }
    resp.on_hover_text(tooltip)
        .on_hover_cursor(CursorIcon::PointingHand)
}

// ---------------------------------------------------------------------------
// Small display pieces
// ---------------------------------------------------------------------------

/// Rounded status pill.
pub fn pill(ui: &mut Ui, text: &str, bg: Color32, fg: Color32) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), semibold(12.0), fg);
    let size = egui::vec2(galley.size().x + 18.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(11), bg);
    ui.painter().galley(
        Pos2::new(rect.min.x + 9.0, rect.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );
    resp
}

pub fn pill_width(ui: &Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), semibold(12.0), Color32::WHITE)
        .size()
        .x
        + 18.0
}

/// Rate chip (↓ / ↑) shown above the list.
pub fn rate_chip(ui: &mut Ui, icon: Icon, text: &str, fg: Color32) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), semibold(13.0), fg);
    let size = egui::vec2(galley.size().x + 12.0 + 14.0 + 6.0 + 12.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(15),
        colors::SURFACE,
        Stroke::new(1.0_f32, colors::BORDER),
        StrokeKind::Inside,
    );
    paint_icon(
        painter,
        Rect::from_center_size(
            Pos2::new(rect.min.x + 12.0 + 7.0, rect.center().y),
            egui::vec2(14.0, 14.0),
        ),
        icon,
        fg,
    );
    painter.galley(
        Pos2::new(
            rect.min.x + 12.0 + 14.0 + 6.0,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        fg,
    );
    resp
}

/// Thin rounded progress bar. `fraction = None` draws an indeterminate sweep.
pub fn progress_bar(ui: &mut Ui, width: f32, fraction: Option<f32>, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width.max(0.0), 6.0), Sense::hover());
    let painter = ui.painter();
    let radius = CornerRadius::same(3);
    painter.rect_filled(rect, radius, colors::TRACK);
    match fraction {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            if f > 0.0 {
                let w = (rect.width() * f).max(6.0);
                painter.rect_filled(
                    Rect::from_min_size(rect.min, egui::vec2(w, rect.height())),
                    radius,
                    color,
                );
            }
        }
        None => {
            let t = ui.input(|i| i.time) as f32;
            let phase = (t / 1.6).fract();
            let seg = rect.width() * 0.4;
            let x0 = rect.min.x - seg + phase * (rect.width() + seg);
            let bar = Rect::from_min_max(
                Pos2::new(x0.max(rect.min.x), rect.min.y),
                Pos2::new((x0 + seg).min(rect.max.x), rect.max.y),
            );
            if bar.width() > 0.0 {
                painter.rect_filled(bar, radius, color);
            }
            ui.ctx().request_repaint();
        }
    }
}
