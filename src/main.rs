// Release builds are a GUI app: no console window behind it.
// Debug builds (`cargo run`) keep the console so logs stay visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod engine;
mod theme;

use eframe::egui;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                // DHT bootstrap retries (e.g. an unreachable bootstrap host) are noise:
                // the session falls back to its cached routing table and other nodes.
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,librqbit_dht=error")),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            .with_title("LaunchTorrent"),
        ..Default::default()
    };

    eframe::run_native(
        "LaunchTorrent",
        options,
        Box::new(|cc| Ok(Box::new(app::LaunchTorrentApp::new(cc)))),
    )
}
