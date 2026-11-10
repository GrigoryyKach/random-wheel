//! Колесо: генератор случайных чисел, математика вращения и отрисовка.

use crate::theme::{self, Pal, hex, mix};
use eframe::egui::{Align2, Color32, Mesh, Painter, Pos2, Shape, Stroke, epaint::TextShape, pos2, vec2};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::hash::{BuildHasher, Hasher};

/// Цвета секторов, как на референсе.
pub const SEGMENTS: [Color32; 11] = [
    hex(0x6366f1),
    hex(0x8b5cf6),
    hex(0xa855f7),
    hex(0xec4899),
    hex(0xf43f5e),
    hex(0xf97316),
    hex(0xf59e0b),
    hex(0x10b981),
    hex(0x14b8a6),
    hex(0x0ea5e9),
    hex(0x3b82f6),
];

/// Цвет сектора: соседние сектора (включая последний и первый) не совпадают.
pub fn segment_color(i: usize, n: usize) -> Color32 {
    let k = SEGMENTS.len();
    let mut idx = i % k;
    if n > 1 && i == n - 1 && idx == 0 {
        idx = k / 2;
    }
    SEGMENTS[idx]
}

// ───────────────────────── случайность ─────────────────────────

/// SplitMix64, засеянный временем и системной случайностью хешера.
/// Победитель выбирается равновероятно до начала анимации;
/// анимация лишь доводит колесо до выбранного сектора.
pub struct Rng(u64);

impl Rng {
    pub fn new() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let h = std::collections::hash_map::RandomState::new().build_hasher().finish();
        Self(t ^ h.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Равномерно в [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Равномерно в [0, n) без смещения (метод Лемира с отбраковкой).
    pub fn below(&mut self, n: usize) -> usize {
        let n = n as u64;
        let threshold = n.wrapping_neg() % n;
        loop {
            let m = (self.next_u64() as u128) * (n as u128);
            if (m as u64) >= threshold {
                return (m >> 64) as usize;
            }
        }
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

// ───────────────────────── вращение ─────────────────────────

/// Плавное замедление к концу.
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Угол поворота (по модулю 2π), при котором указатель стоит на доле `frac` сектора `i`.
pub fn rotation_for(i: usize, frac: f32, n: usize) -> f32 {
    let s = TAU / n as f32;
    (-(i as f32 + frac) * s).rem_euclid(TAU)
}

// ───────────────────────── отрисовка ─────────────────────────

fn dir(a: f32) -> eframe::egui::Vec2 {
    vec2(a.cos(), a.sin())
}

/// Мягкое свечение вокруг колеса: кольцо с прозрачностью от `inner` к нулю.
fn glow(painter: &Painter, c: Pos2, r0: f32, r1: f32, color: Color32) {
    let mut mesh = Mesh::default();
    let steps = 96;
    for k in 0..=steps {
        let a = k as f32 / steps as f32 * TAU;
        mesh.colored_vertex(c + dir(a) * r0, color);
        mesh.colored_vertex(c + dir(a) * r1, Color32::TRANSPARENT);
    }
    for k in 0..steps {
        let i = (k * 2) as u32;
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 3, i + 2);
    }
    painter.add(Shape::mesh(mesh));
}

fn fit_label(painter: &Painter, name: &str, size: f32, max_w: f32, color: Color32) -> std::sync::Arc<eframe::egui::Galley> {
    let font = theme::semi(size);
    let g = painter.layout_no_wrap(name.to_string(), font.clone(), color);
    if g.size().x <= max_w {
        return g;
    }
    let chars: Vec<char> = name.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let s: String = chars[..mid].iter().collect::<String>() + "…";
        if painter.layout_no_wrap(s, font.clone(), color).size().x <= max_w { lo = mid } else { hi = mid - 1 }
    }
    painter.layout_no_wrap(chars[..lo].iter().collect::<String>() + "…", font, color)
}

pub struct WheelState<'a> {
    pub entries: &'a [String],
    pub rot: f32,
    pub highlight: Option<usize>,
    /// Отклонение указателя (рад), когда мимо него проходит граница сектора.
    pub kick: f32,
}

pub fn paint(painter: &Painter, c: Pos2, r: f32, st: &WheelState, p: &Pal) {
    let n = st.entries.len();
    let glow_a = if p.dark { 70 } else { 45 };
    glow(painter, c + vec2(0.0, 6.0), r - 4.0, r + 34.0, Color32::from_rgba_unmultiplied(95, 100, 255, glow_a));

    if n == 0 {
        painter.circle_filled(c, r, p.card);
        painter.circle_stroke(c, r, Stroke::new(1.0, p.line));
        painter.text(c + vec2(0.0, r * 0.32), Align2::CENTER_CENTER, "Добавьте участников справа", theme::reg(15.0), p.muted);
        paint_pointer(painter, c, r, 0.0, p);
        paint_hub(painter, c, r, p);
        return;
    }

    let s = TAU / n as f32;
    let start = st.rot - FRAC_PI_2;
    let dim = |i: usize, col: Color32| match st.highlight {
        Some(h) if h != i => mix(col, p.bg, 0.55),
        _ => col,
    };

    if n == 1 {
        painter.circle_filled(c, r, dim(0, segment_color(0, 1)));
    } else {
        let steps = ((s / TAU) * 220.0).ceil().max(2.0) as usize;
        for i in 0..n {
            let a0 = start + i as f32 * s;
            let mut pts = Vec::with_capacity(steps + 2);
            pts.push(c);
            for k in 0..=steps {
                pts.push(c + dir(a0 + s * k as f32 / steps as f32) * r);
            }
            painter.add(Shape::convex_polygon(pts, dim(i, segment_color(i, n)), Stroke::NONE));
        }
        if n <= 120 {
            let sep = Stroke::new(1.2, Color32::from_white_alpha(70));
            for i in 0..n {
                painter.line_segment([c, c + dir(start + i as f32 * s) * r], sep);
            }
        }
    }

    // Подписи: вдоль радиуса, прижаты к ободу.
    let label_r = r * 0.9;
    let max_w = r * 0.66;
    let chord = 2.0 * (r * 0.62) * (s / 2.0).min(FRAC_PI_2).sin();
    let size = (chord * 0.5).min(r * 0.085).clamp(0.0, 24.0);
    if n == 1 {
        let g = fit_label(painter, &st.entries[0], (r * 0.09).clamp(13.0, 26.0), r * 1.4, Color32::WHITE);
        painter.galley(c + vec2(-g.size().x / 2.0, -r * 0.5 - g.size().y / 2.0), g, Color32::WHITE);
    } else if size >= 9.0 {
        for (i, name) in st.entries.iter().enumerate() {
            let a = start + (i as f32 + 0.5) * s;
            let anchor = c + dir(a) * label_r;
            let alpha = if st.highlight.is_some_and(|h| h != i) { 150 } else { 255 };
            let shade = fit_label(painter, name, size, max_w, Color32::from_black_alpha(alpha / 4));
            let g = fit_label(painter, name, size, max_w, Color32::from_white_alpha(alpha));
            let half = vec2(g.size().x, g.size().y / 2.0);
            painter.add(
                TextShape::new(anchor - half + dir(a + FRAC_PI_2) * 1.0, shade, Color32::BLACK)
                    .with_angle_and_anchor(a, Align2::RIGHT_CENTER),
            );
            painter.add(TextShape::new(anchor - half, g, Color32::WHITE).with_angle_and_anchor(a, Align2::RIGHT_CENTER));
        }
    }

    // Обводка победившего сектора.
    if let (Some(h), true) = (st.highlight, n > 1) {
        let a0 = start + h as f32 * s;
        let steps = ((s / TAU) * 220.0).ceil().max(2.0) as usize;
        let mut pts = vec![c];
        for k in 0..=steps {
            pts.push(c + dir(a0 + s * k as f32 / steps as f32) * (r - 1.5));
        }
        painter.add(Shape::closed_line(pts, Stroke::new(3.0, Color32::from_white_alpha(230))));
    }

    painter.circle_stroke(c, r, Stroke::new(3.0, Color32::from_white_alpha(if p.dark { 210 } else { 255 })));
    painter.circle_stroke(c, r + 1.5, Stroke::new(1.0, Color32::from_black_alpha(if p.dark { 60 } else { 20 })));
    paint_pointer(painter, c, r, st.kick, p);
    paint_hub(painter, c, r, p);
}

fn paint_pointer(painter: &Painter, c: Pos2, r: f32, kick: f32, p: &Pal) {
    let size = (r * 0.06).clamp(12.0, 20.0);
    let pivot = pos2(c.x, c.y - r - size * 0.55);
    let rot = |v: eframe::egui::Vec2| {
        let (s, co) = kick.sin_cos();
        pivot + vec2(v.x * co - v.y * s, v.x * s + v.y * co)
    };
    let pts = [vec2(-size, -size * 0.35), vec2(size, -size * 0.35), vec2(0.0, size * 1.45)];
    let shadow: Vec<Pos2> = pts.iter().map(|&v| rot(v) + vec2(0.0, 2.5)).collect();
    painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha(70), Stroke::NONE));
    let body: Vec<Pos2> = pts.iter().map(|&v| rot(v)).collect();
    painter.add(Shape::convex_polygon(body, p.accent, Stroke::new(2.0, Color32::WHITE)));
}

fn paint_hub(painter: &Painter, c: Pos2, r: f32, p: &Pal) {
    let h = (r * 0.045).clamp(9.0, 18.0);
    painter.circle_filled(c + vec2(0.0, 2.0), h + 4.0, Color32::from_black_alpha(60));
    painter.circle_filled(c, h + 3.0, Color32::WHITE);
    painter.circle_filled(c, h, p.accent2);
    painter.circle_filled(c + vec2(-h * 0.15, -h * 0.15), h * 0.7, mix(p.accent2, p.accent, 0.8));
    painter.circle_filled(c + vec2(-h * 0.35, -h * 0.35), h * 0.28, Color32::from_white_alpha(110));
}
