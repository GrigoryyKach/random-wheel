use std::f32::consts::TAU;
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Frame, Id, Key, Layout, Margin, Modifiers, Order, Pos2, Rect, RichText,
    ScrollArea, Sense, Shape, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::config::{Config, View};
use crate::i18n::{self, Lang};
use crate::nimvoro::{self, UpdateState, Updater};
use crate::strings::{self, S};
use crate::theme::{self, Btn, Pal, Textures};
use crate::wheel::{self, Rng, WheelState};

pub fn parse_entries(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

struct Spin {
    start: f64,
    dur: f64,
    from: f32,
    to: f32,
    winner: usize,
    name: String,
    n: usize,
    list_from: usize,
    list_steps: usize,
}

struct Outcome {
    winner: usize,
    name: String,
    at: f64,
}

struct Particle {
    pos: Pos2,
    vel: Vec2,
    rot: f32,
    vr: f32,
    size: Vec2,
    color: Color32,
}

pub struct App {
    cfg: Config,
    lang: Lang,
    rot: f32,
    spin: Option<Spin>,
    outcome: Option<Outcome>,
    highlight: Option<usize>,
    list_cursor: usize,
    rng: Rng,
    tex: Textures,
    file_label: Option<String>,
    import_error: Option<String>,
    confetti: Vec<Particle>,
    dirty_since: Option<f64>,
    focus_mode: bool,
    updater: Updater,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, mut cfg: Config) -> Self {
        theme::install(&cc.egui_ctx, cfg.theme);
        // Starting language: config, then the launcher marker, then English; persisted for next time.
        let lang = i18n::resolve(cfg.language.as_deref());
        let mut dirty_since = None;
        if cfg.language.as_deref() != Some(lang.code()) {
            cfg.language = Some(lang.code().to_string());
            dirty_since = Some(0.0);
        }
        if cfg.fresh && cfg.text.is_empty() {
            cfg.text = strings::TABLE.get(lang).sample_names.to_string();
        }
        let mut rng = Rng::new();
        let rot = rng.f32() * TAU;
        let updater = Updater::new("random-wheel", "GrigoryyKach/random-wheel", env!("CARGO_PKG_VERSION"));
        if cfg.check_updates {
            let ctx = cc.egui_ctx.clone();
            updater.check(move || ctx.request_repaint());
        }
        Self {
            cfg,
            lang,
            rot,
            spin: None,
            outcome: None,
            highlight: None,
            list_cursor: 0,
            rng,
            tex: Textures::new(&cc.egui_ctx),
            file_label: None,
            import_error: None,
            confetti: Vec::new(),
            dirty_since,
            focus_mode: false,
            updater,
        }
    }

    fn s(&self) -> &'static S {
        strings::TABLE.get(self.lang)
    }

    fn touch(&mut self, ctx: &egui::Context) {
        self.dirty_since = Some(ctx.input(|i| i.time));
    }

    fn can_spin(&self, entries: &[String]) -> bool {
        entries.len() >= 2 && self.spin.is_none() && self.outcome.is_none()
    }

    // ───────────────────────── draw ─────────────────────────

    fn start_spin(&mut self, ctx: &egui::Context, entries: &[String]) {
        if !self.can_spin(entries) {
            return;
        }
        let n = entries.len();
        // The winner is decided here with equal probability; the animation only leads to it.
        let winner = self.rng.below(n);
        let frac = 0.15 + 0.7 * self.rng.f32();
        let from = self.rot.rem_euclid(TAU);
        let target = wheel::rotation_for(winner, frac, n);
        let turns = (self.cfg.spin_secs * 1.3).round().max(3.0);
        let to = from + (target - from).rem_euclid(TAU) + TAU * turns;

        let list_from = self.list_cursor % n;
        let laps = ((self.cfg.spin_secs * 7.0) / n as f32).ceil().max(1.0) as usize;
        let list_steps = n * laps + (winner + n - list_from) % n;

        self.highlight = None;
        self.rot = from;
        self.spin = Some(Spin {
            start: ctx.input(|i| i.time),
            dur: self.cfg.spin_secs as f64,
            from,
            to,
            winner,
            name: entries[winner].clone(),
            n,
            list_from,
            list_steps,
        });
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
    }

    fn finish_spin(&mut self, ctx: &egui::Context, sp: Spin) {
        let now = ctx.input(|i| i.time);
        self.rot = sp.to.rem_euclid(TAU);
        self.list_cursor = sp.winner;
        self.highlight = Some(sp.winner);
        self.cfg.history.insert(0, sp.name.clone());
        self.cfg.history.truncate(15);
        self.touch(ctx);
        self.outcome = Some(Outcome { winner: sp.winner, name: sp.name, at: now });
        if self.cfg.confetti {
            self.spawn_confetti(ctx.content_rect());
        }
    }

    fn remove_entry(&mut self, idx: usize) {
        let mut k = 0;
        let mut out = Vec::new();
        for line in self.cfg.text.lines() {
            if !line.trim().is_empty() {
                let skip = k == idx;
                k += 1;
                if skip {
                    continue;
                }
            }
            out.push(line);
        }
        self.cfg.text = out.join("\n");
    }

    fn close_outcome(&mut self, ctx: &egui::Context, remove: bool) {
        if let Some(o) = self.outcome.take() {
            if remove || self.cfg.remove_after {
                self.remove_entry(o.winner);
                self.highlight = None;
                self.list_cursor = 0;
                self.touch(ctx);
            }
        }
    }

    fn toggle_focus(&mut self, ctx: &egui::Context) {
        self.focus_mode = !self.focus_mode;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.focus_mode));
    }

    fn handle_keys(&mut self, ctx: &egui::Context, entries: &[String]) {
        let none = Modifiers::NONE;
        if self.outcome.is_some() {
            let close = ctx.input_mut(|i| {
                i.consume_key(none, Key::Enter) || i.consume_key(none, Key::Escape) || i.consume_key(none, Key::Space)
            });
            if close {
                self.close_outcome(ctx, false);
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(none, Key::F11)) {
            self.toggle_focus(ctx);
        }
        let typing = ctx.memory(|m| m.focused().is_some());
        if self.focus_mode && !typing && ctx.input_mut(|i| i.consume_key(none, Key::Escape)) {
            self.toggle_focus(ctx);
        }
        if !typing && self.can_spin(entries) && ctx.input_mut(|i| i.consume_key(none, Key::Space) || i.consume_key(none, Key::Enter)) {
            self.start_spin(ctx, entries);
        }
    }

    // ───────────────────────── import ─────────────────────────

    fn import_file(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.s().dlg_title)
            .add_filter(self.s().dlg_lists, &["txt", "csv"])
            .add_filter(self.s().dlg_all, &["*"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                let is_csv = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"));
                let names = crate::import::parse(&crate::import::decode(&bytes), is_csv);
                if names.is_empty() {
                    self.import_error = Some(self.s().err_no_names.into());
                } else {
                    self.cfg.text = names.join("\n");
                    self.import_error = None;
                    self.highlight = None;
                    self.touch(ctx);
                }
                self.file_label = path.file_name().map(|n| n.to_string_lossy().into_owned());
            }
            Err(e) => self.import_error = Some(format!("{}: {e}", self.s().err_read)),
        }
    }

    fn shuffle_entries(&mut self, ctx: &egui::Context) {
        let mut v = parse_entries(&self.cfg.text);
        self.rng.shuffle(&mut v);
        self.cfg.text = v.join("\n");
        self.highlight = None;
        self.touch(ctx);
    }

    fn sort_entries(&mut self, ctx: &egui::Context) {
        let mut v = parse_entries(&self.cfg.text);
        v.sort_by_key(|s| s.to_lowercase());
        self.cfg.text = v.join("\n");
        self.highlight = None;
        self.touch(ctx);
    }

    // ───────────────────────── confetti ─────────────────────────

    fn spawn_confetti(&mut self, screen: Rect) {
        for _ in 0..170 {
            let r = &mut self.rng;
            self.confetti.push(Particle {
                pos: pos2(screen.left() + r.f32() * screen.width(), screen.top() - r.f32() * screen.height() * 0.6 - 10.0),
                vel: vec2((r.f32() - 0.5) * 160.0, 120.0 + r.f32() * 220.0),
                rot: r.f32() * TAU,
                vr: (r.f32() - 0.5) * 12.0,
                size: vec2(6.0 + r.f32() * 5.0, 3.5 + r.f32() * 3.0),
                color: wheel::SEGMENTS[r.below(wheel::SEGMENTS.len())],
            });
        }
    }

    fn step_confetti(&mut self, ctx: &egui::Context, dt: f32) {
        if self.confetti.is_empty() {
            return;
        }
        let bottom = ctx.content_rect().bottom() + 20.0;
        let t = ctx.input(|i| i.time) as f32;
        for (k, p) in self.confetti.iter_mut().enumerate() {
            p.vel.y = (p.vel.y + 260.0 * dt).min(420.0);
            let sway = (t * 3.0 + k as f32).sin() * 40.0;
            p.pos += vec2(p.vel.x + sway, p.vel.y) * dt;
            p.rot += p.vr * dt;
        }
        self.confetti.retain(|p| p.pos.y < bottom);
        ctx.request_repaint();
    }

    fn paint_confetti(&self, ctx: &egui::Context) {
        if self.confetti.is_empty() {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::unique("confetti")));
        for p in &self.confetti {
            let (s, c) = p.rot.sin_cos();
            // Squash along one axis so the paper seems to tumble.
            let flip = (p.rot * 1.7).cos().abs().max(0.15);
            let hx = vec2(c, s) * p.size.x / 2.0;
            let hy = vec2(-s, c) * p.size.y / 2.0 * flip;
            let pts = vec![p.pos - hx - hy, p.pos + hx - hy, p.pos + hx + hy, p.pos - hx + hy];
            painter.add(Shape::convex_polygon(pts, p.color, Stroke::NONE));
        }
    }

    // ───────────────────────── main area ─────────────────────────

    fn main_view(&mut self, ui: &mut Ui, p: &Pal, entries: &[String]) {
        let ctx = ui.ctx().clone();
        let t = self.s();
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(t.title).font(theme::bold(28.0)).color(p.text));
            ui.add_space(-4.0);
            let sub = if entries.is_empty() {
                t.add_hint.to_string()
            } else {
                i18n::plural(self.lang, entries.len(), &t.in_draw)
            };
            ui.label(RichText::new(sub).font(theme::reg(14.0)).color(p.muted));
            ui.add_space(10.0);
            self.toolbar(ui, p);
            ui.add_space(14.0);
        });

        let avail = ui.available_rect_before_wrap();
        let btn_h = 48.0;
        let area = Rect::from_min_max(avail.min, pos2(avail.max.x, avail.max.y - btn_h - 22.0));
        let width = match self.cfg.view {
            View::Wheel => self.wheel_view(ui, area, p, entries),
            View::List => self.list_view(ui, area, p, entries),
        };

        let can = self.can_spin(entries);
        let label = if self.spin.is_some() {
            t.spinning
        } else if entries.len() < 2 {
            t.need_two
        } else if self.cfg.view == View::List {
            t.pick_random
        } else {
            t.spin_wheel
        };
        let bw = width.clamp(280.0, 540.0);
        let brect = Rect::from_center_size(pos2(avail.center().x, avail.max.y - btn_h / 2.0), vec2(bw, btn_h));
        let resp = theme::gradient_button(ui, brect, Id::unique("spin"), label, Some(theme::icon_spin), can, &self.tex, p);
        if resp.clicked() {
            self.start_spin(&ctx, entries);
        }
        ui.allocate_rect(avail, Sense::hover());
    }

    /// The List / Wheel switch and the full-screen button.
    fn toolbar(&mut self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        let items = [(View::List, t.list, theme::icon_list as fn(_, _, _)), (View::Wheel, t.wheel, theme::icon_wheel)];
        let font = theme::semi(13.5);
        let widths: Vec<f32> = items
            .iter()
            .map(|(_, l, _)| ui.painter().layout_no_wrap(l.to_string(), font.clone(), p.text).size().x + 12.0 + 16.0 + 8.0 + 14.0)
            .collect();
        let seg_w = widths.iter().sum::<f32>() + 8.0;
        let h = 38.0;
        let (rect, _) = ui.allocate_exact_size(vec2(seg_w + 8.0 + h, h), Sense::hover());
        let seg = Rect::from_min_size(rect.min, vec2(seg_w, h));
        ui.painter().rect(seg, 11.0, p.sidebar, Stroke::new(1.0, p.line), StrokeKind::Inside);

        let mut x = seg.left() + 4.0;
        for ((view, label, icon), w) in items.into_iter().zip(widths) {
            let r = Rect::from_min_size(pos2(x, seg.top() + 4.0), vec2(w, h - 8.0));
            let resp = ui.interact(r, Id::unique(("seg", label)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let active = self.cfg.view == view;
            let painter = ui.painter();
            if active {
                if !p.dark {
                    painter.add(theme::shadow(1, 3, Color32::from_black_alpha(25)).as_shape(r, 8.0));
                }
                painter.rect_filled(r, 8.0, p.card);
            } else if resp.hovered() {
                painter.rect_filled(r, 8.0, p.hover);
            }
            let col = if active || resp.hovered() { p.text } else { p.muted };
            icon(painter, pos2(r.left() + 12.0 + 8.0, r.center().y), if active { p.accent } else { col });
            painter.text(pos2(r.left() + 12.0 + 16.0 + 8.0, r.center().y), Align2::LEFT_CENTER, label, font.clone(), col);
            if resp.clicked() && !active {
                self.cfg.view = view;
                self.dirty_since = Some(ui.input(|i| i.time));
            }
            x += w;
        }

        let fr = Rect::from_min_size(pos2(seg.right() + 8.0, rect.top()), Vec2::splat(h));
        let tip = if self.focus_mode { t.exit_fullscreen } else { t.fullscreen };
        let resp = ui.interact(fr, Id::unique("fullscreen"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
        if resp.hovered() {
            ui.painter().rect_filled(fr, 9.0, p.hover);
        }
        theme::icon_fullscreen(ui.painter(), fr.center(), if resp.hovered() { p.text } else { p.muted }, self.focus_mode);
        if resp.clicked() {
            let ctx = ui.ctx().clone();
            self.toggle_focus(&ctx);
        }
    }

    fn wheel_view(&mut self, ui: &mut Ui, area: Rect, p: &Pal, entries: &[String]) -> f32 {
        let d = (area.width().min(area.height()) - 30.0).clamp(160.0, 760.0);
        let r = d / 2.0;
        let c = area.center();
        let resp = ui.interact(Rect::from_center_size(c, Vec2::splat(d)), Id::unique("wheel"), Sense::click());
        let inside = resp.hover_pos().is_some_and(|pos| pos.distance(c) <= r);
        let can = self.can_spin(entries);
        if can && inside {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() && inside {
            let ctx = ui.ctx().clone();
            self.start_spin(&ctx, entries);
        }

        // The pointer "catches" on segment borders while the wheel spins.
        let kick = match &self.spin {
            Some(sp) if sp.n <= 60 => {
                let s = TAU / sp.n as f32;
                let frac = ((-self.rot).rem_euclid(TAU) / s).fract();
                if frac > 0.8 { -(frac - 0.8) / 0.2 * 0.38 } else { 0.0 }
            }
            _ => 0.0,
        };
        let st = WheelState { entries, rot: self.rot, highlight: self.highlight, kick, empty_text: self.s().wheel_empty };
        wheel::paint(ui.painter(), c, r, &st, p);
        d
    }

    fn list_view(&mut self, ui: &mut Ui, area: Rect, p: &Pal, entries: &[String]) -> f32 {
        let w = area.width().min(560.0);
        let col = Rect::from_center_size(area.center(), vec2(w, area.height()));
        let mut child = ui.new_child(UiBuilder::new().max_rect(col).layout(Layout::top_down(Align::Min)));

        if entries.is_empty() {
            let r = Rect::from_min_size(col.min, vec2(w, 120.0));
            let stroke = Stroke::new(1.0, p.line);
            let c = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            for k in 0..4 {
                child.painter().extend(Shape::dashed_line(&[c[k], c[k + 1]], stroke, 6.0, 4.0));
            }
            child.painter().text(r.center(), Align2::CENTER_CENTER, self.s().list_empty, theme::reg(13.5), p.muted);
            return w;
        }

        let spinning = self.spin.is_some();
        let cursor = self.list_cursor;
        ScrollArea::vertical().id_salt("list").auto_shrink([false; 2]).show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for (i, name) in entries.iter().enumerate() {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
                let active = spinning && i == cursor;
                let win = !spinning && self.highlight == Some(i);
                let painter = ui.painter();
                let (num_bg, num_col, text_col);
                if win {
                    painter.add(theme::shadow(6, 18, Color32::from_rgba_unmultiplied(95, 100, 255, 60)).as_shape(r, 12.0));
                    theme::gradient_rect(painter, r, 12.0, &self.tex.accent);
                    (num_bg, num_col, text_col) = (Color32::from_white_alpha(50), Color32::WHITE, Color32::WHITE);
                } else if active {
                    painter.rect(r, 12.0, theme::mix(p.card, p.accent, 0.14), Stroke::new(1.5, p.accent), StrokeKind::Inside);
                    (num_bg, num_col, text_col) = (p.accent, Color32::WHITE, p.text);
                } else {
                    let dimmed = self.highlight.is_some() && !spinning;
                    painter.rect(r, 12.0, p.card, Stroke::new(1.0, p.line), StrokeKind::Inside);
                    (num_bg, num_col, text_col) = (p.accent_soft, p.accent, if dimmed { p.muted } else { p.text });
                }
                let badge = Rect::from_min_size(pos2(r.left() + 12.0, r.center().y - 11.0), vec2(30.0, 22.0));
                painter.rect_filled(badge, 11.0, num_bg);
                painter.text(badge.center(), Align2::CENTER_CENTER, (i + 1).to_string(), theme::bold(12.0), num_col);
                let clip = Rect::from_min_max(pos2(badge.right() + 12.0, r.top()), pos2(r.right() - 12.0, r.bottom()));
                let font = if win { theme::semi(15.0) } else { theme::reg(14.5) };
                painter.with_clip_rect(clip).text(pos2(clip.left(), r.center().y), Align2::LEFT_CENTER, name, font, text_col);
                if active {
                    ui.scroll_to_rect(r, Some(Align::Center));
                }
            }
        });
        w
    }

    // ───────────────────────── settings panel ─────────────────────────

    fn settings_panel(&mut self, ui: &mut Ui, p: &Pal, entries: &[String]) {
        let ctx = ui.ctx().clone();
        let t = self.s();
        ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);

            // Panel header, like the brand block in Smart Switcher.
            ui.horizontal(|ui| {
                theme::logo(ui, 34.0, "⚙", &self.tex);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.add_space(1.0);
                    ui.label(RichText::new(t.settings).font(theme::semi(15.0)).color(p.text));
                    ui.add_space(-8.0);
                    ui.label(RichText::new("Random Wheel").font(theme::reg(12.0)).color(p.muted));
                });
            });
            ui.add_space(14.0);
            self.update_banner(ui, p, &ctx);

            // ── Participants
            ui.horizontal(|ui| {
                theme::caps(ui, t.participants, p);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(entries.len().to_string()).font(theme::semi(12.0)).color(p.muted));
                });
            });
            ui.add_space(2.0);
            self.entries_editor(ui, p, &ctx);
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                if theme::button(ui, t.browse, Btn::Soft, p).clicked() && self.spin.is_none() {
                    self.import_file(&ctx);
                }
                ui.add_space(2.0);
                let (txt, col) = match &self.file_label {
                    Some(f) => (f.as_str(), p.text),
                    None => (t.no_file, p.muted),
                };
                ui.add(egui::Label::new(RichText::new(txt).font(theme::reg(13.0)).color(col)).truncate());
            });
            if let Some(err) = &self.import_error {
                ui.label(RichText::new(err).font(theme::reg(12.5)).color(theme::hex(0xe5484d)));
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.file_types).font(theme::reg(12.5)).color(p.muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let enabled = self.spin.is_none() && entries.len() > 1;
                    if theme::button(ui, t.sort_az, Btn::Link, p).on_hover_text(t.sort_tip).clicked() && enabled {
                        self.sort_entries(&ctx);
                    }
                    if theme::button(ui, t.shuffle, Btn::Link, p).clicked() && enabled {
                        self.shuffle_entries(&ctx);
                    }
                });
            });

            // ── Draw
            theme::divider(ui, p);
            theme::caps(ui, t.draw, p);
            ui.add_space(2.0);
            ui.label(RichText::new(t.win_text).font(theme::reg(14.0)).color(p.text));
            let r = ui.add(
                TextEdit::singleline(&mut self.cfg.prompt)
                    .hint_text(t.win_hint)
                    .desired_width(f32::INFINITY)
                    .margin(Margin::symmetric(12, 9))
                    .font(theme::reg(14.0)),
            );
            if r.changed() {
                self.touch(&ctx);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.spin_duration).font(theme::reg(14.0)).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    theme::pill(ui, &t.seconds.replace("{n}", &(self.cfg.spin_secs as i32).to_string()), p);
                });
            });
            if theme::slider(ui, &mut self.cfg.spin_secs, 2.0, 12.0, 1.0, p).changed() {
                self.touch(&ctx);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.quick).font(theme::reg(12.0)).color(p.muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(t.suspense).font(theme::reg(12.0)).color(p.muted));
                });
            });

            // ── Behavior
            theme::divider(ui, p);
            theme::caps(ui, t.behavior, p);
            ui.add_space(2.0);
            let mut remove = self.cfg.remove_after;
            if switch_row(ui, p, t.remove_after, t.remove_after_hint, &mut remove) {
                self.cfg.remove_after = remove;
                self.touch(&ctx);
            }
            ui.add_space(6.0);
            let mut confetti = self.cfg.confetti;
            if switch_row(ui, p, t.confetti, t.confetti_hint, &mut confetti) {
                self.cfg.confetti = confetti;
                self.touch(&ctx);
            }

            // ── History
            theme::divider(ui, p);
            ui.horizontal(|ui| {
                theme::caps(ui, t.history, p);
                if !self.cfg.history.is_empty() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::button(ui, t.clear_history, Btn::Link, p).clicked() {
                            self.cfg.history.clear();
                            self.touch(&ctx);
                        }
                    });
                }
            });
            ui.add_space(2.0);
            if self.cfg.history.is_empty() {
                theme::hint(ui, t.history_empty, p);
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (k, name) in self.cfg.history.iter().enumerate() {
                        chip(ui, name, k == 0, p);
                    }
                });
            }
            // ── Interface: language and theme
            theme::divider(ui, p);
            theme::caps(ui, t.appearance, p);
            ui.add_space(2.0);
            let mut lang = self.lang;
            if i18n::language_row(ui, &mut lang, p) {
                self.lang = lang;
                self.cfg.language = Some(lang.code().to_string());
                self.touch(&ctx);
            }
            ui.add_space(4.0);
            let mut mode = self.cfg.theme;
            if i18n::theme_row(ui, &mut mode, self.lang, p) {
                self.cfg.theme = mode;
                theme::apply(&ctx, mode);
                self.touch(&ctx);
            }

            // ── Updates
            let u = nimvoro::strings(self.lang);
            theme::divider(ui, p);
            theme::caps(ui, u.updates, p);
            ui.add_space(2.0);
            let mut check = self.cfg.check_updates;
            if switch_row(ui, p, u.check_on_start, u.check_on_start_hint, &mut check) {
                self.cfg.check_updates = check;
                self.touch(&ctx);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                theme::hint(ui, &self.updater.status_text(self.lang), p);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !self.updater.is_checking() && theme::button(ui, u.check_now, Btn::Link, p).clicked() {
                        let c = ctx.clone();
                        self.updater.check(move || c.request_repaint());
                    }
                });
            });
            ui.add_space(10.0);
        });
    }

    /// "Version X is available" banner above the settings.
    fn update_banner(&mut self, ui: &mut Ui, p: &Pal, ctx: &egui::Context) {
        let UpdateState::Available { version, .. } = self.updater.state() else { return };
        let u = nimvoro::strings(self.lang);
        Frame::new()
            .fill(p.accent_soft)
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(u.available.replace("{v}", &version)).font(theme::semi(13.5)).color(p.text));
                        ui.add_space(-6.0);
                        theme::hint(ui, &u.you_have.replace("{v}", self.updater.current), p);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::button(ui, u.update_btn, Btn::Soft, p).clicked() && self.updater.apply() {
                            self.cfg.save();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
            });
        ui.add_space(12.0);
    }

    /// Multi-line participants field with a Clear button in the corner.
    fn entries_editor(&mut self, ui: &mut Ui, p: &Pal, ctx: &egui::Context) {
        let enabled = self.spin.is_none() && self.outcome.is_none();
        let hint = self.s().entries_hint;
        let mut te_resp = None;
        let frame = Frame::new()
            .fill(p.bg)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(10)
            .inner_margin(Margin { left: 12, right: 8, top: 8, bottom: 8 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ScrollArea::vertical().id_salt("entries").min_scrolled_height(200.0).max_height(200.0).show(ui, |ui| {
                    let r = ui.add_enabled(
                        enabled,
                        TextEdit::multiline(&mut self.cfg.text)
                            .frame(Frame::NONE)
                            .hint_text(hint)
                            .desired_width(f32::INFINITY)
                            .desired_rows(9)
                            .font(theme::reg(14.5)),
                    );
                    te_resp = Some(r);
                });
            });
        let rect = frame.response.rect;
        if let Some(r) = &te_resp {
            if r.changed() {
                self.highlight = None;
                self.touch(ctx);
            }
            if r.has_focus() {
                ui.painter().rect_stroke(rect.expand(1.5), 11.5, Stroke::new(3.0, p.accent_soft), StrokeKind::Outside);
                ui.painter().rect_stroke(rect, 10.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
            }
        }
        if !self.cfg.text.trim().is_empty() && enabled {
            let size = vec2(72.0, 24.0);
            let br = Rect::from_min_size(pos2(rect.right() - size.x - 14.0, rect.top() + 8.0), size);
            if theme::button_at(ui, br, Id::unique("clear"), self.s().clear, Btn::Small, p).clicked() {
                self.cfg.text.clear();
                self.highlight = None;
                self.file_label = None;
                self.touch(ctx);
            }
        }
    }

    // ───────────────────────── winner dialog ─────────────────────────

    fn winner_modal(&mut self, ctx: &egui::Context, p: &Pal) {
        let Some(o) = &self.outcome else { return };
        let now = ctx.input(|i| i.time);
        let a = ((now - o.at) / 0.25).clamp(0.0, 1.0) as f32;
        let e = wheel::ease_out(a);
        if a < 1.0 {
            ctx.request_repaint();
        }
        let name = o.name.clone();
        let t = self.s();
        let screen = ctx.content_rect();

        let dim = egui::Area::new(Id::unique("dim")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha((150.0 * e) as u8));
            r
        });
        let mut close = dim.inner.clicked();
        let mut remove = false;

        egui::Area::new(Id::unique("winner"))
            .order(Order::Tooltip)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, (1.0 - e) * 24.0))
            .show(ctx, |ui| {
                ui.multiply_opacity(e);
                Frame::new()
                    .fill(p.card)
                    .stroke(Stroke::new(1.0, p.line))
                    .corner_radius(20)
                    .inner_margin(Margin::symmetric(32, 28))
                    .shadow(theme::shadow(20, 60, Color32::from_black_alpha(90)))
                    .show(ui, |ui| {
                        ui.set_width(380.0);
                        ui.vertical_centered(|ui| {
                            // Medal badge.
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                            let painter = ui.painter();
                            painter.add(theme::shadow(6, 20, Color32::from_rgba_unmultiplied(95, 100, 255, 90)).as_shape(r, 32.0));
                            theme::gradient_rect(painter, r, 32.0, &self.tex.accent);
                            painter.text(r.center(), Align2::CENTER_CENTER, "★", theme::bold(30.0), Color32::WHITE);
                            ui.add_space(10.0);

                            let prompt = if self.cfg.prompt.trim().is_empty() { t.winner_default } else { self.cfg.prompt.trim() };
                            ui.label(RichText::new(prompt).font(theme::reg(15.0)).color(p.muted));
                            ui.add_space(2.0);
                            ui.add(egui::Label::new(RichText::new(&name).font(theme::bold(34.0)).color(p.accent)).wrap());
                            if self.cfg.remove_after {
                                ui.add_space(2.0);
                                theme::hint(ui, t.will_be_removed, p);
                            }
                            ui.add_space(18.0);

                            let h = 42.0;
                            let total = if self.cfg.remove_after { 180.0 } else { 370.0 };
                            let (row, _) = ui.allocate_exact_size(vec2(total, h), Sense::hover());
                            let done_rect = if self.cfg.remove_after {
                                row
                            } else {
                                let ghost = Rect::from_min_size(row.min, vec2(180.0, h));
                                if theme::button_at(ui, ghost, Id::unique("w-remove"), t.remove_from_list, Btn::Ghost, p).clicked() {
                                    remove = true;
                                    close = true;
                                }
                                Rect::from_min_max(pos2(row.right() - 180.0, row.top()), row.max)
                            };
                            if theme::gradient_button(ui, done_rect, Id::unique("w-done"), t.done, None, true, &self.tex, p).clicked() {
                                close = true;
                            }
                        });
                    });
            });

        if close {
            self.close_outcome(ctx, remove);
        }
    }
}

fn switch_row(ui: &mut Ui, p: &Pal, title: &str, hint: &str, value: &mut bool) -> bool {
    let mut changed = false;
    let w = ui.available_width();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(w - 58.0);
            ui.label(RichText::new(title).font(theme::reg(14.0)).color(p.text));
            ui.add_space(-6.0);
            theme::hint(ui, hint, p);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = theme::switch(ui, value, p).changed();
        });
    });
    changed
}

fn chip(ui: &mut Ui, text: &str, latest: bool, p: &Pal) {
    let font = theme::reg(13.0);
    let g = ui.painter().layout_no_wrap(text.to_string(), font, if latest { p.accent } else { p.text });
    let w = (g.size().x + 22.0).min(ui.available_width().max(60.0));
    let (r, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    let (fill, stroke) = if latest { (p.accent_soft, Stroke::NONE) } else { (p.bg, Stroke::new(1.0, p.line)) };
    ui.painter().rect(r, 13.0, fill, stroke, StrokeKind::Inside);
    let clip = r.shrink2(vec2(11.0, 0.0));
    ui.painter().with_clip_rect(clip).galley(pos2(clip.left(), r.center().y - g.size().y / 2.0), g, p.text);
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        let dt = ctx.input(|i| i.stable_dt).min(0.05);

        let mut done = false;
        if let Some(sp) = &self.spin {
            let t = ((now - sp.start) / sp.dur).clamp(0.0, 1.0) as f32;
            let e = wheel::ease_out(t);
            self.rot = sp.from + (sp.to - sp.from) * e;
            self.list_cursor = (sp.list_from + ((sp.list_steps as f32) * e).floor() as usize) % sp.n;
            done = t >= 1.0;
            ctx.request_repaint();
        }
        if done {
            let sp = self.spin.take().expect("spin");
            self.finish_spin(ctx, sp);
        }

        self.step_confetti(ctx, dt);

        if let Some(t0) = self.dirty_since {
            if now - t0 > 0.5 {
                self.cfg.save();
                self.dirty_since = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(550));
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = Pal::of(ui.visuals().dark_mode);
        let entries = parse_entries(&self.cfg.text);
        self.handle_keys(&ctx, &entries);
        // Debug only: RANDOM_WHEEL_AUTOSPIN=1 starts a spin one second after launch.
        #[cfg(debug_assertions)]
        if ctx.input(|i| i.time) > 1.0
            && self.cfg.history.is_empty()
            && std::env::var_os("RANDOM_WHEEL_AUTOSPIN").is_some()
            && self.spin.is_none()
            && self.outcome.is_none()
        {
            self.start_spin(&ctx, &entries);
        }
        #[cfg(debug_assertions)]
        if std::env::var_os("RANDOM_WHEEL_AUTOSPIN").is_some() {
            ctx.request_repaint();
        }

        if !self.focus_mode {
            egui::Panel::right("settings")
                .exact_size(340.0)
                .resizable(false)
                .frame(Frame::new().fill(p.sidebar).inner_margin(Margin { left: 22, right: 12, top: 22, bottom: 12 }))
                .show(ui, |ui| self.settings_panel(ui, &p, &entries));
        }
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(32, 24)))
            .show(ui, |ui| self.main_view(ui, &p, &entries));

        if self.outcome.is_some() {
            self.winner_modal(&ctx, &p);
        }
        self.paint_confetti(&ctx);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if self.dirty_since.is_some() {
            self.cfg.save();
        }
    }
}
