#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod import;
mod nimvoro;
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
        .with_icon(std::sync::Arc::new(app_icon(64)));
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("Random Wheel", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, cfg)))))
}

/// Иконка рисуется кодом: градиентный квадрат как логотип Smart Switcher, внутри — колесо.
fn app_icon(size: u32) -> egui::IconData {
    let s = size as f32;
    let (a, b) = ([0x4f, 0x6b, 0xff], [0x8a, 0x5b, 0xff]);
    let smooth = |d: f32| (0.5 - d).clamp(0.0, 1.0); // 1 внутри, 0 снаружи, 1px сглаживание
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Скруглённый квадрат.
            let r = s * 0.24;
            let qx = (px - s / 2.0).abs() - (s / 2.0 - r);
            let qy = (py - s / 2.0).abs() - (s / 2.0 - r);
            let d_box = (qx.max(0.0).hypot(qy.max(0.0))) + qx.max(qy).min(0.0) - r;
            let cover = smooth(d_box);
            let t = (px + py) / (2.0 * s);
            let mut col = [0f32; 3];
            for i in 0..3 {
                col[i] = a[i] as f32 + (b[i] as f32 - a[i] as f32) * t;
            }
            // Колесо: обод, три спицы и ступица.
            let (dx, dy) = (px - s / 2.0, py - s * 0.54);
            let dist = dx.hypot(dy);
            let rim = s * 0.29;
            let mut white = smooth((dist - rim).abs() - s * 0.045);
            white = white.max(smooth(dist - s * 0.075));
            for k in 0..3 {
                let ang = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / 3.0;
                let (ux, uy) = (ang.cos(), ang.sin());
                let along = (dx * ux + dy * uy).clamp(0.0, rim);
                let perp = (dx - ux * along).hypot(dy - uy * along);
                white = white.max(smooth(perp - s * 0.028));
            }
            // Указатель сверху.
            let ty = py - (s * 0.54 - rim - s * 0.07);
            if (0.0..s * 0.12).contains(&ty) {
                let half = (s * 0.12 - ty) * 0.6;
                white = white.max(smooth((px - s / 2.0).abs() - half));
            }
            for c in &mut col {
                *c += (255.0 - *c) * white;
            }
            rgba.extend_from_slice(&[col[0] as u8, col[1] as u8, col[2] as u8, (cover * 255.0) as u8]);
        }
    }
    egui::IconData { rgba, width: size, height: size }
}
