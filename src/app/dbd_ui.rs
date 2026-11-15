//! The "Dead by Daylight" tab: killer picker, round panel and actions. The rules live in `dbd.rs`.

use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, Frame, Id, Layout, Margin, Painter, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextEdit, Ui, pos2, vec2,
};

use super::App;
use crate::config::Tab;
use crate::dbd::{self, BanBlock, Killer, KillerList, State};
use crate::dbd_io;
use crate::dbd_strings::{self, D};
use crate::i18n::{self, Lang};
use crate::theme::{self, Btn, Pal};

/// State of the tab that is not part of the rules: the list, the search box, open sections.
pub struct Dbd {
    pub(super) state: State,
    base: KillerList,
    pub(super) killers: Vec<Killer>,
    search: String,
    history_open: bool,
    confirm_reset: bool,
    custom_text: String,
    fetched: Option<Arc<Mutex<Option<KillerList>>>>,
    /// Names on the wheel while a spin and its result window are on screen.
    pub(super) frozen: Vec<String>,
}

impl Dbd {
    pub fn new(ctx: &egui::Context) -> Self {
        let state = dbd_io::load_state();
        let base = dbd_io::load_list();
        let c = ctx.clone();
        let fetched = Some(dbd_io::fetch_list(base.version, move || c.request_repaint()));
        let mut d = Self {
            state,
            base,
            killers: Vec::new(),
            search: String::new(),
            history_open: false,
            confirm_reset: false,
            custom_text: String::new(),
            fetched,
            frozen: Vec::new(),
        };
        d.rebuild();
        d
    }

    fn rebuild(&mut self) {
        self.killers = dbd::merged(&self.base, &self.state.custom);
    }

    fn save(&self) {
        dbd_io::save_state(&self.state);
    }

    /// Ids on the wheel, in list order.
    pub(super) fn eligible_ids(&self) -> Vec<String> {
        self.state.eligible(&self.killers).into_iter().map(String::from).collect()
    }

    /// Picks up a newer list fetched in the background.
    pub(super) fn poll(&mut self) {
        let new = self.fetched.as_ref().and_then(|s| s.lock().ok().and_then(|mut g| g.take()));
        if let Some(l) = new {
            self.base = KillerList::newest(self.base.clone(), Some(l));
            self.rebuild();
            self.fetched = None;
        }
    }
}

struct Row {
    id: String,
    name: String,
    in_pool: bool,
    locked: bool,
    immune: bool,
    delisted: bool,
    counter: u32,
}

fn badge(painter: &Painter, right: f32, cy: f32, text: &str, fill: Color32, col: Color32) -> f32 {
    let g = painter.layout_no_wrap(text.to_string(), theme::semi(10.5), col);
    let w = g.size().x + 12.0;
    let r = Rect::from_min_size(pos2(right - w, cy - 9.0), vec2(w, 18.0));
    painter.rect_filled(r, 9.0, fill);
    painter.galley(pos2(r.left() + 6.0, cy - g.size().y / 2.0), g, col);
    r.left() - 5.0
}

fn red() -> Color32 {
    theme::hex(0xe5484d)
}

impl App {
    pub(super) fn d(&self) -> &'static D {
        dbd_strings::TABLE.get(self.lang)
    }

    fn english(&self) -> bool {
        self.lang == Lang::En
    }

    fn dbd_busy(&self) -> bool {
        self.spin.is_some() || self.outcome.is_some()
    }

    /// Names on the wheel: the frozen snapshot while a round is being shown, otherwise live.
    pub(super) fn dbd_entries(&self) -> Vec<String> {
        if !self.dbd.frozen.is_empty() {
            return self.dbd.frozen.clone();
        }
        let en = self.english();
        let s = &self.dbd.state;
        self.dbd.killers.iter().filter(|k| s.in_pool(&k.id) && !s.is_banned(&k.id)).map(|k| k.name(en).to_string()).collect()
    }

    pub(super) fn dbd_commit(&mut self, id: &str) {
        self.dbd.state.commit(&self.dbd.killers, id, dbd_io::now_string());
        self.dbd.save();
    }

    // ───────────────────────── header ─────────────────────────

    /// Wheel / Dead by Daylight switch above the title.
    pub(super) fn tab_switch(&mut self, ui: &mut Ui, p: &Pal) {
        if self.focus_mode {
            return;
        }
        let d = self.d();
        let items = [(Tab::Wheel, d.tab_wheel), (Tab::Dbd, d.tab_dbd)];
        let font = theme::semi(13.5);
        let widths: Vec<f32> =
            items.iter().map(|(_, l)| ui.painter().layout_no_wrap(l.to_string(), font.clone(), p.text).size().x + 30.0).collect();
        let h = 38.0;
        let (seg, _) = ui.allocate_exact_size(vec2(widths.iter().sum::<f32>() + 8.0, h), Sense::hover());
        ui.painter().rect(seg, 11.0, p.sidebar, Stroke::new(1.0, p.line), StrokeKind::Inside);
        let mut x = seg.left() + 4.0;
        for ((tab, label), w) in items.into_iter().zip(widths) {
            let r = Rect::from_min_size(pos2(x, seg.top() + 4.0), vec2(w, h - 8.0));
            let resp = ui.interact(r, Id::unique(("tab", label)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let active = self.cfg.tab == tab;
            let painter = ui.painter();
            if active {
                if !p.dark {
                    painter.add(theme::shadow(1, 3, Color32::from_black_alpha(25)).as_shape(r, 8.0));
                }
                painter.rect_filled(r, 8.0, p.card);
            } else if resp.hovered() {
                painter.rect_filled(r, 8.0, p.hover);
            }
            let col = if active { p.accent } else if resp.hovered() { p.text } else { p.muted };
            painter.text(r.center(), Align2::CENTER_CENTER, label, font.clone(), col);
            if resp.clicked() && !active && !self.dbd_busy() {
                self.cfg.tab = tab;
                self.highlight = None;
                self.dirty_since = Some(ui.input(|i| i.time));
            }
            x += w;
        }
    }

    // ───────────────────────── main area ─────────────────────────

    pub(super) fn main_view_dbd(&mut self, ui: &mut Ui, p: &Pal, entries: &[String]) {
        let ctx = ui.ctx().clone();
        let d = self.d();
        let (in_pool, banned) = {
            let s = &self.dbd.state;
            (s.pool_ids(&self.dbd.killers).len(), s.banned_count(&self.dbd.killers))
        };
        let on_wheel = in_pool - banned;
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            self.tab_switch(ui, p);
            ui.add_space(10.0);
            ui.label(RichText::new(d.title).font(theme::bold(28.0)).color(p.text));
            ui.add_space(-4.0);
            let sub = d.counts.replace("{a}", &in_pool.to_string()).replace("{b}", &banned.to_string()).replace("{c}", &on_wheel.to_string());
            ui.label(RichText::new(sub).font(theme::reg(14.0)).color(p.muted));
            ui.add_space(10.0);
        });
        self.pool_chips(ui, p);
        ui.add_space(8.0);

        let avail = ui.available_rect_before_wrap();
        let btn_h = 48.0;
        let area = Rect::from_min_max(avail.min, pos2(avail.max.x, avail.max.y - btn_h - 22.0));
        let width = self.wheel_view(ui, area, p, entries);

        let can = self.can_spin(entries);
        let label = if self.spin.is_some() { d.spinning } else { d.spin };
        let bw = width.clamp(280.0, 540.0);
        let brect = Rect::from_center_size(pos2(avail.center().x, avail.max.y - btn_h / 2.0), vec2(bw, btn_h));
        if !can && !self.dbd_busy() {
            let text = if in_pool == 0 { d.pool_empty } else { d.nothing_to_spin };
            ui.painter().text(pos2(avail.center().x, brect.top() - 12.0), Align2::CENTER_CENTER, text, theme::reg(12.5), p.muted);
        }
        let resp = theme::gradient_button(ui, brect, Id::unique("dbd-spin"), label, Some(theme::icon_spin), can, &self.tex, p);
        if resp.clicked() {
            self.start_spin(&ctx, entries);
        }
        ui.allocate_rect(avail, Sense::hover());
    }

    /// The current pool as chips, each with a Ban / Unban toggle.
    fn pool_chips(&mut self, ui: &mut Ui, p: &Pal) {
        let d = self.d();
        let en = self.english();
        let busy = self.dbd_busy();
        let s = &self.dbd.state;
        let rows: Vec<(String, String, bool, Option<BanBlock>)> = self
            .dbd
            .killers
            .iter()
            .filter(|k| s.in_pool(&k.id))
            .map(|k| (k.id.clone(), k.name(en).to_string(), s.is_banned(&k.id), s.ban_block(&k.id)))
            .collect();
        if rows.is_empty() {
            return;
        }
        let mut action: Option<(String, bool)> = None;
        ScrollArea::vertical().id_salt("dbd_chips").max_height(96.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for (id, name, banned, block) in &rows {
                    let text_col = if *banned { p.muted } else { p.text };
                    let g = ui.painter().layout_no_wrap(name.clone(), theme::reg(13.0), text_col);
                    let label = if *banned { d.unban } else { d.ban };
                    let bg = ui.painter().layout_no_wrap(label.to_string(), theme::semi(11.5), p.accent);
                    let bw = bg.size().x + 14.0;
                    let w = (g.size().x + 12.0 + bw + 10.0).min(ui.available_width().max(120.0));
                    let (r, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
                    let (fill, stroke) = if *banned {
                        (theme::mix(p.card, red(), 0.14), Stroke::new(1.0, theme::mix(p.line, red(), 0.5)))
                    } else {
                        (p.card, Stroke::new(1.0, p.line))
                    };
                    ui.painter().rect(r, 14.0, fill, stroke, StrokeKind::Inside);
                    let clip = Rect::from_min_max(pos2(r.left() + 12.0, r.top()), pos2(r.right() - bw - 8.0, r.bottom()));
                    ui.painter().with_clip_rect(clip).galley(pos2(clip.left(), r.center().y - g.size().y / 2.0), g, text_col);

                    let br = Rect::from_min_size(pos2(r.right() - bw - 5.0, r.top() + 4.0), vec2(bw, 20.0));
                    let allowed = *banned || block.is_none();
                    let sense = if allowed && !busy { Sense::click() } else { Sense::hover() };
                    let resp = ui.interact(br, Id::unique(("dbd-ban", id)), sense);
                    let (bfill, bcol) = if !allowed {
                        (Color32::from_rgba_unmultiplied(127, 127, 127, 30), p.muted)
                    } else if resp.hovered() {
                        (theme::mix(p.accent_soft, p.accent, 0.15), p.accent)
                    } else {
                        (p.accent_soft, p.accent)
                    };
                    ui.painter().rect_filled(br, 10.0, bfill);
                    ui.painter().text(br.center(), Align2::CENTER_CENTER, label, theme::semi(11.5), bcol);
                    if !allowed {
                        let tip = if *block == Some(BanBlock::Immune) { d.tip_immune } else { d.tip_locked };
                        resp.on_hover_text(tip);
                    } else {
                        if resp.clicked() {
                            action = Some((id.clone(), !*banned));
                        }
                        resp.on_hover_text(if *banned { d.tip_unban } else { d.tip_ban }).on_hover_cursor(CursorIcon::PointingHand);
                    }
                }
            });
        });
        if let Some((id, ban)) = action {
            if ban {
                self.dbd.state.ban(&id);
            } else {
                self.dbd.state.unban(&id);
            }
            self.dbd.save();
        }
    }

    // ───────────────────────── side panel ─────────────────────────

    pub(super) fn dbd_panel(&mut self, ui: &mut Ui, p: &Pal) {
        let ctx = ui.ctx().clone();
        let d = self.d();
        let en = self.english();
        let busy = self.dbd_busy();
        ScrollArea::vertical().id_salt("dbd_side").auto_shrink([false; 2]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 8.0);
            self.update_banner(ui, p, &ctx);

            // ── Picker
            theme::caps(ui, d.killers, p);
            ui.add_space(2.0);
            let r = ui.add(
                TextEdit::singleline(&mut self.dbd.search)
                    .hint_text(d.search)
                    .desired_width(f32::INFINITY)
                    .margin(Margin::symmetric(12, 8))
                    .font(theme::reg(14.0)),
            );
            let _ = r;
            let rows: Vec<Row> = {
                let s = &self.dbd.state;
                self.dbd
                    .killers
                    .iter()
                    .filter(|k| dbd::matches_search(k, &self.dbd.search, en))
                    .map(|k| Row {
                        id: k.id.clone(),
                        name: k.name(en).to_string(),
                        in_pool: s.in_pool(&k.id),
                        locked: s.is_locked(&k.id),
                        immune: s.is_immune(&k.id),
                        delisted: k.delisted,
                        counter: s.counter(&k.id),
                    })
                    .collect()
            };
            ui.add_space(4.0);
            let mut bulk = 0;
            ui.horizontal(|ui| {
                if theme::button(ui, d.all, Btn::Small, p).clicked() {
                    bulk = 1;
                }
                if theme::button(ui, d.none, Btn::Small, p).clicked() {
                    bulk = 2;
                }
                if theme::button(ui, d.invert, Btn::Small, p).clicked() {
                    bulk = 3;
                }
            });
            ui.add_space(4.0);
            let mut toggle: Option<String> = None;
            if rows.is_empty() {
                theme::hint(ui, d.no_matches, p);
            } else {
                let rh = 30.0;
                ScrollArea::vertical().id_salt("dbd_pick").max_height(310.0).auto_shrink([false, true]).show_rows(
                    ui,
                    rh,
                    rows.len(),
                    |ui, range| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        for row in &rows[range] {
                            if self.killer_row(ui, p, d, row, busy) {
                                toggle = Some(row.id.clone());
                            }
                        }
                    },
                );
            }
            if !busy {
                let ids = rows.iter().map(|r| r.id.as_str());
                match bulk {
                    1 => self.dbd.state.select_all(ids),
                    2 => self.dbd.state.select_none(ids),
                    3 => self.dbd.state.invert(ids),
                    _ => {}
                }
                if let Some(id) = &toggle {
                    self.dbd.state.toggle_pool(id);
                }
                if bulk != 0 || toggle.is_some() {
                    self.dbd.save();
                }
            }

            // ── Round actions
            theme::divider(ui, p);
            theme::caps(ui, d.actions, p);
            ui.add_space(2.0);
            let can_undo = self.dbd.state.can_undo() && !busy;
            ui.horizontal_wrapped(|ui| {
                ui.scope(|ui| {
                    if !can_undo {
                        ui.multiply_opacity(0.45);
                    }
                    if theme::button(ui, d.undo, Btn::Ghost, p).on_hover_text(d.undo_tip).clicked() && can_undo {
                        self.dbd.state.undo();
                        self.dbd.save();
                        self.highlight = None;
                    }
                });
                if theme::button(ui, d.reset, Btn::Ghost, p).clicked() && !busy {
                    self.dbd.confirm_reset = true;
                }
            });
            if self.dbd.confirm_reset {
                ui.add_space(6.0);
                Frame::new().fill(p.accent_soft).corner_radius(10.0).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(d.reset_ask).font(theme::reg(13.0)).color(p.text));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if theme::button(ui, d.reset_yes, Btn::Soft, p).clicked() {
                            self.dbd.state.reset_counters();
                            self.dbd.save();
                            self.dbd.confirm_reset = false;
                        }
                        if theme::button(ui, d.cancel, Btn::Ghost, p).clicked() {
                            self.dbd.confirm_reset = false;
                        }
                    });
                });
            }

            // ── History
            ui.add_space(8.0);
            let n = self.dbd.state.core.history.len();
            let arrow = if self.dbd.history_open { "▾" } else { "▸" };
            if theme::button(ui, &format!("{arrow} {} ({n})", d.history), Btn::Link, p).clicked() {
                self.dbd.history_open = !self.dbd.history_open;
            }
            if self.dbd.history_open {
                if n == 0 {
                    theme::hint(ui, d.history_empty, p);
                } else {
                    self.history_list(ui, p, d, en);
                }
            }

            // ── Custom killers
            theme::divider(ui, p);
            theme::caps(ui, d.custom, p);
            theme::hint(ui, d.custom_hint, p);
            ui.add_space(2.0);
            let mut add = false;
            ui.horizontal(|ui| {
                let r = ui.add(
                    TextEdit::singleline(&mut self.dbd.custom_text)
                        .hint_text(d.custom_name)
                        .desired_width(ui.available_width() - 120.0)
                        .margin(Margin::symmetric(12, 8))
                        .font(theme::reg(14.0)),
                );
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if theme::button(ui, d.custom_add, Btn::Soft, p).clicked() || enter {
                    add = true;
                }
            });
            if add {
                let text = std::mem::take(&mut self.dbd.custom_text);
                if let Some(id) = self.dbd.state.add_custom(&text, dbd_io::unix_now()) {
                    self.dbd.state.set_in_pool(&id, true);
                    self.dbd.rebuild();
                    self.dbd.save();
                } else {
                    self.dbd.custom_text = text;
                }
            }
            let mut remove: Option<String> = None;
            for k in &self.dbd.state.custom {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&k.en).font(theme::reg(13.5)).color(p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::button(ui, d.remove, Btn::Link, p).clicked() {
                            remove = Some(k.id.clone());
                        }
                    });
                });
            }
            if let Some(id) = remove {
                self.dbd.state.remove_custom(&id);
                self.dbd.rebuild();
                self.dbd.save();
            }

            // ── Interface
            theme::divider(ui, p);
            theme::caps(ui, self.s().appearance, p);
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

            ui.add_space(10.0);
            theme::hint(ui, &d.list_version.replace("{n}", &self.dbd.base.version.to_string()), p);
            theme::hint(ui, d.disclaimer, p);
            ui.add_space(10.0);
        });
    }

    /// One checkable killer card. Returns true when it was clicked.
    fn killer_row(&self, ui: &mut Ui, p: &Pal, d: &D, row: &Row, busy: bool) -> bool {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        let hov = resp.hovered() && !row.locked;
        let checked = row.in_pool && !row.locked;
        let (fill, stroke) = if checked {
            (theme::mix(p.card, p.accent, 0.12), Stroke::new(1.0, p.accent))
        } else if hov {
            (p.hover, Stroke::new(1.0, p.line))
        } else {
            (p.card, Stroke::new(1.0, p.line))
        };
        let painter = ui.painter();
        painter.rect(r, 8.0, fill, stroke, StrokeKind::Inside);
        let dim = row.locked;
        let text_col = if dim { theme::mix(p.muted, p.bg, 0.3) } else { p.text };

        // Checkbox.
        let cb = Rect::from_center_size(pos2(r.left() + 16.0, r.center().y), vec2(16.0, 16.0));
        if checked {
            painter.rect_filled(cb, 4.0, p.accent);
            let st = Stroke::new(2.0, Color32::WHITE);
            painter.line_segment([pos2(cb.left() + 3.5, cb.center().y), pos2(cb.center().x - 0.5, cb.bottom() - 4.0)], st);
            painter.line_segment([pos2(cb.center().x - 0.5, cb.bottom() - 4.0), pos2(cb.right() - 3.0, cb.top() + 4.0)], st);
        } else {
            painter.rect_stroke(cb, 4.0, Stroke::new(1.3, if dim { p.line } else { p.muted }), StrokeKind::Inside);
        }

        // Right side: ban pips, then badges (drawn right to left).
        let mut right = r.right() - 10.0;
        for i in 0..2 {
            let c = pos2(right - 4.0 - i as f32 * 12.0, r.center().y);
            if row.counter > i {
                painter.circle_filled(c, 4.0, red());
            } else {
                painter.circle_stroke(c, 3.6, Stroke::new(1.2, p.muted));
            }
        }
        right -= 30.0;
        let neutral = Color32::from_rgba_unmultiplied(127, 127, 127, 40);
        if row.delisted {
            right = badge(painter, right, r.center().y, d.delisted, neutral, p.muted);
        }
        if row.locked {
            right = badge(painter, right, r.center().y, d.locked, p.accent_soft, p.accent);
        }
        if row.immune {
            right = badge(painter, right, r.center().y, d.immune, theme::mix(p.card, theme::hex(0x10b981), 0.25), theme::hex(0x0e9f6e));
        }
        let clip = Rect::from_min_max(pos2(cb.right() + 8.0, r.top()), pos2(right, r.bottom()));
        painter.with_clip_rect(clip).text(pos2(clip.left(), r.center().y), Align2::LEFT_CENTER, &row.name, theme::reg(13.5), text_col);

        let clicked = resp.clicked() && !row.locked && !busy;
        let mut tip = row.name.clone();
        if row.locked {
            tip = format!("{tip}: {}", d.tip_locked);
        } else if row.immune {
            tip = format!("{tip}: {}", d.tip_immune);
        }
        let resp = resp.on_hover_text(tip);
        if !row.locked {
            resp.on_hover_cursor(CursorIcon::PointingHand);
        }
        clicked
    }

    fn history_list(&self, ui: &mut Ui, p: &Pal, d: &D, en: bool) {
        let name_of = |id: &str| -> String {
            self.dbd.killers.iter().find(|k| k.id == id).map(|k| k.name(en).to_string()).unwrap_or_else(|| id.to_string())
        };
        ScrollArea::vertical().id_salt("dbd_hist").max_height(240.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            for h in &self.dbd.state.core.history {
                let banned = if h.banned.is_empty() {
                    d.hist_no_bans.to_string()
                } else {
                    format!("{}: {}", d.hist_banned, h.banned.iter().map(|b| name_of(b)).collect::<Vec<_>>().join(", "))
                };
                let detail = format!("{} · {}", d.hist_pool.replace("{n}", &h.pool_size.to_string()), banned);
                Frame::new().fill(p.card).stroke(Stroke::new(1.0, p.line)).corner_radius(8.0).inner_margin(Margin::symmetric(10, 6)).show(
                    ui,
                    |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(name_of(&h.winner)).font(theme::semi(13.5)).color(p.accent));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(&h.when).font(theme::reg(11.5)).color(p.muted));
                            });
                        });
                        ui.add(egui::Label::new(RichText::new(detail).font(theme::reg(12.0)).color(p.muted)).wrap());
                    },
                );
            }
        });
    }
}
