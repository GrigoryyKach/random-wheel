#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod i18n;
mod import;
mod nimvoro;
mod strings;
mod theme;
mod wheel;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let cfg = config::Config::load();
    let viewport = egui::ViewportBuilder::default()
        .with_title("Random Wheel")
        .with_app_id("local.random-wheel")
        .with_inner_size([1180.0, 780.0])
        .with_min_inner_size([900.0, 620.0])
        .with_icon(std::sync::Arc::new(app_icon()));
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("Random Wheel", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, cfg)))))
}

/// Window icon from `assets/icon.png` (the same artwork that is embedded in the exe).
fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap_or_default()
}
