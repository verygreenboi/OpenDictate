//! Drawing: sidebar, Home insights, History and Dictionary. The Settings window
//! lives in `settings_view`.

use crate::app::{DictateApp, Page, SettingsTab};
use crate::settings::{Action, DictWord, KeepHistory, SpeechEngine, TextEngine};
use crate::stats;
use crate::theme::{self, b, d, m, para, t};
use chrono::{Datelike, Duration, Local};
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Margin, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn draw(app: &mut DictateApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    egui::Panel::left("sidebar")
        .exact_size(216.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(theme::BG).inner_margin(Margin { left: 10, right: 8, top: 10, bottom: 10 }))
        .show(ui, |ui| sidebar(app, ui));

    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG)).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let full = ui.available_width();
            let width = (full - 80.0).clamp(560.0, 1080.0);
            let pad = ((full - width) / 2.0).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.add_space(36.0);
                    match app.page {
                        Page::Home => home(app, ui, width),
                        Page::History => history(app, ui),
                        Page::Dictionary => dictionary(app, ui),
                    }
                    ui.add_space(40.0);
                });
            });
        });
    });

    if app.settings_tab.is_some() {
        crate::settings_view::settings_window(app, &ctx);
    }
    crate::settings_view::capture_shortcut(app, &ctx);
    toast(app, &ctx);
}

// ---------------------------------------------------------------------------
// Sidebar

fn sidebar(app: &mut DictateApp, ui: &mut egui::Ui) {
    // The kit's .sidepanel: a 3px ink outline over a hard ink shadow. Inset so
    // the shadow has somewhere to fall inside the pane.
    let rect = ui.max_rect().shrink2(vec2(7.0, 10.0));
    let p = ui.painter();
    p.rect_filled(rect.translate(vec2(5.0, 6.0)), 18.0, theme::INK);
    p.rect_filled(rect, 18.0, theme::SURFACE);
    p.rect_stroke(rect, 18.0, Stroke::new(3.0, theme::INK), egui::StrokeKind::Inside);

    let inner = rect.shrink2(vec2(10.0, 12.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)));
    let ui = &mut ui;

    // .sp-head
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        logo(ui);
        ui.add_space(4.0);
        ui.label(d("OPEN DICTATE", 15.0, theme::TEXT));
    });
    ui.add_space(4.0);

    let hist = app.history.len();
    let words = app.s.dictionary.len();
    sp_cap(ui, "PAGES");
    for (page, icon, label, badge) in [
        (Page::Home, "home", "Home", None),
        (Page::History, "clock", "History", (hist > 0).then(|| hist.to_string())),
        (Page::Dictionary, "book", "Dictionary", (words > 0).then(|| words.to_string())),
    ] {
        if sp_item(ui, icon, label, app.page == page, badge).clicked() {
            app.page = page;
        }
    }

    // .sp-foot, laid out upwards from the bottom of the panel.
    let (speech_ok, _) = app.speech_ready();
    let (text_ok, _) = app.text_ready();
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        let btn = egui::Button::new(b("Manage models", 14.0, theme::ON_FILL))
            .fill(theme::ACCENT)
            .corner_radius(14)
            .min_size(vec2(ui.available_width(), 38.0));
        if ui.add(btn).clicked() {
            app.settings_tab = Some(SettingsTab::Models);
            app.refresh_health(ui.ctx());
        }
        ui.add_space(8.0);
        // The kit lists Settings in the foot, under the meter, rather than as
        // a group of its own. The modal has its own side panel for the rest.
        if sp_item(ui, "gear", "Settings", app.settings_tab.is_some(), None).clicked() {
            app.settings_tab = Some(SettingsTab::General);
        }
        ui.add_space(4.0);

        // .sp-meter: speech is what dictation needs, text only sweetens it.
        let filled = if speech_ok { 0.5 } else { 0.0 } + if text_ok { 0.5 } else { 0.0 };
        let (m, _) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 9.0), Sense::hover());
        let m = m.translate(vec2(6.0, 0.0));
        let p = ui.painter();
        p.rect_filled(Rect::from_min_size(m.min, vec2(m.width() * filled, m.height())), 4.0, theme::INK);
        p.rect_stroke(m, 4.0, Stroke::new(2.0, theme::INK), egui::StrokeKind::Inside);
        ui.add_space(6.0);

        // .sp-foot-row: a word on the left, the figure on the right.
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
        let p = ui.painter();
        p.text(pos2(r.left() + 6.0, r.center().y), Align2::LEFT_CENTER, "MODELS", FontId::new(9.0, theme::bold()), theme::FAINT);
        let (word, tone) = match (speech_ok, text_ok) {
            (true, true) => ("BOTH READY", theme::TEXT),
            (true, false) => ("SPEECH ONLY", theme::AMBER),
            _ => ("NOT READY", theme::RED),
        };
        p.text(pos2(r.right() - 6.0, r.center().y), Align2::RIGHT_CENTER, word, FontId::new(12.0, theme::display()), tone);
        ui.add_space(10.0);
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.5), Sense::hover());
        ui.painter().rect_filled(line, 0.0, theme::LINE);
    });
}

/// Small line icons drawn with the painter (the original uses monochrome outline icons).
pub fn draw_icon(p: &egui::Painter, name: &str, c: egui::Pos2, color: Color32) {
    // Lucide glyphs, keyed by the names the call sites already use, so adding
    // an icon here is a one line change and nothing else moves.
    let glyph = match name {
        "home" => crate::icon::HOUSE,
        "clock" => crate::icon::HISTORY,
        "book" => crate::icon::BOOK,
        "mic" => crate::icon::MIC,
        "gear" => crate::icon::SETTINGS,
        "help" | "info" => crate::icon::HELP,
        "spark" => crate::icon::SPARKLES,
        "close" => crate::icon::X,
        // Anything unknown still shows itself, which is easier to spot than a blank.
        other => other,
    };
    p.text(c, Align2::CENTER_CENTER, glyph, FontId::proportional(17.0), color);
}

/// The kit's .sp-logo: a 30px ink-bordered square in yellow, with its own
/// small hard shadow, holding the three bars mark.
fn logo(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r.translate(vec2(2.0, 3.0)), 9.0, theme::INK);
    p.rect_filled(r, 9.0, theme::AMBER);
    p.rect_stroke(r, 9.0, Stroke::new(2.5, theme::INK), egui::StrokeKind::Inside);
    let c = r.center();
    for (i, h) in [8.0, 14.0, 10.0].iter().enumerate() {
        let x = c.x - 5.0 + i as f32 * 5.0;
        p.rect_filled(Rect::from_center_size(pos2(x, c.y), vec2(3.0, *h)), 1.5, theme::PAPER);
    }
}

/// The kit's .sp-cap: a group heading, tiny and widely tracked.
fn sp_cap(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 13.0), Sense::hover());
    let spaced: String = text.chars().flat_map(|c| [c, ' ']).collect();
    ui.painter().text(
        pos2(r.left() + 10.0, r.center().y),
        Align2::LEFT_CENTER,
        spaced.trim_end(),
        FontId::new(9.0, theme::bold()),
        theme::FAINT,
    );
    ui.add_space(3.0);
}

/// The kit's .sp-item. The chosen row is filled with ink like the primary
/// button, and carries a yellow shadow rather than an ink one.
pub fn nav_item(ui: &mut egui::Ui, icon: &str, label: &str, active: bool) -> egui::Response {
    sp_item(ui, icon, label, active, None)
}

fn sp_item(ui: &mut egui::Ui, icon: &str, label: &str, active: bool, badge: Option<String>) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let p = ui.painter();
    if active {
        p.rect_filled(r.translate(vec2(3.0, 4.0)), 14.0, theme::AMBER);
        p.rect_filled(r, 14.0, theme::INK);
    } else if resp.hovered() {
        p.rect_filled(r, 14.0, theme::PAPER);
        p.rect_stroke(r, 14.0, Stroke::new(2.5, theme::INK), egui::StrokeKind::Inside);
    }
    let fg = if active { theme::PAPER } else { theme::TEXT };
    draw_icon(p, icon, pos2(r.left() + 20.0, r.center().y), fg);
    p.text(pos2(r.left() + 40.0, r.center().y), Align2::LEFT_CENTER, label, FontId::new(13.0, theme::medium()), fg);
    if let Some(n) = badge {
        // .sp-badge: outlined in whatever the row's colour is, display face.
        let galley = p.layout_no_wrap(n, FontId::new(11.0, theme::display()), fg);
        let w = (galley.rect.width() + 14.0).max(22.0);
        let br = Rect::from_center_size(pos2(r.right() - 12.0 - w / 2.0, r.center().y), vec2(w, 19.0));
        p.rect_stroke(br, 9.0, Stroke::new(2.0, fg), egui::StrokeKind::Inside);
        p.galley(pos2(br.center().x - galley.rect.width() / 2.0, br.center().y - galley.rect.height() / 2.0), galley, fg);
    }
    ui.add_space(4.0);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn status_line(ui: &mut egui::Ui, dot: Color32, what: &str, label: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 3.5, dot);
        ui.label(b(what, 12.5, theme::TEXT));
        ui.add(egui::Label::new(t(label, 12.5, theme::MUTED)).truncate());
    });
}

pub fn icon_button(ui: &mut egui::Ui, icon: &str, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, theme::NAV_ACTIVE);
    }
    draw_icon(ui.painter(), icon, r.center(), if resp.hovered() { theme::TEXT } else { theme::MUTED });
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------------------------------------------------------------------
// Home

fn home(app: &mut DictateApp, ui: &mut egui::Ui, width: f32) {
    let right_w = 186.0;
    let gap = (width * 0.085).clamp(24.0, 92.0);
    let main_w = width - right_w - gap;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.set_width(main_w);
            ui.label(d("Say it, skip the typing", 46.0, theme::TEXT));
            ui.add_space(26.0);
            insights(app, ui, main_w);
        });
        ui.add_space(gap);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.set_width(right_w);
            ui.add_space(98.0);
            right_column(app, ui);
        });
    });
}

fn insights(app: &mut DictateApp, ui: &mut egui::Ui, w: f32) {
    // Dotted backdrop, filled in once we know how tall the cards are.
    let bg_idx = ui.painter().add(egui::Shape::Noop);
    let top = ui.cursor().top();
    let left = ui.cursor().left();
    let s = &app.stats;
    let mut share = false;
    ui.horizontal(|ui| {
        ui.add_space(0.0);
        ui.label(d("Insights", 24.0, theme::MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let btn = egui::Button::new(m(format!("{} Share", crate::icon::SHARE), 14.0, theme::TEXT))
                .fill(theme::SURFACE)
                .stroke(Stroke::new(2.5, theme::INK))
                .corner_radius(16);
            share = ui.add(btn).clicked();
        });
    });
    if share {
        let summary = format!(
            "{} words dictated, {} saved, {:.0} WPM, {} active days.",
            s.words,
            duration_text(s.minutes_saved(app.s.typing_wpm)),
            s.wpm(),
            s.active_days()
        );
        ui.ctx().copy_text(summary);
        app.notify("Insights copied to clipboard", false);
    }
    ui.add_space(8.0);

    let s = &app.stats;
    let card_w = (w - 16.0) / 2.0;
    let saved = s.minutes_saved(app.s.typing_wpm);
    let total = s.seconds / 60.0;
    let cards = [
        (vec![(compact(s.words as f64), "words")], "Words dictated"),
        (duration_parts(saved), "Time saved"),
        (vec![(format!("{:.0}", s.wpm()), "WPM")], "Average dictation speed"),
        (duration_parts(total), "Total dictation time"),
    ];
    for row in cards.chunks(2) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            for (parts, label) in row {
                stat_card(ui, card_w, parts, label);
            }
        });
        ui.add_space(8.0);
    }
    activity_card(app, ui, w);
    ui.add_space(8.0);
    personalization_card(app, ui, w);

    let rect = Rect::from_min_max(pos2(left - 12.0, top - 12.0), pos2(left + w + 12.0, ui.cursor().top() + 4.0));
    let mut shapes = vec![egui::Shape::rect_filled(rect, 14.0, theme::PAPER)];
    let mut y = rect.top() + 4.0;
    while y < rect.bottom() - 2.0 {
        let mut x = rect.left() + 4.0;
        while x < rect.right() - 2.0 {
            shapes.push(egui::Shape::rect_filled(Rect::from_min_size(pos2(x, y), vec2(1.0, 1.0)), 0.0, Color32::from_rgba_unmultiplied(0xF4, 0xF3, 0xEF, 20)));
            x += 6.0;
        }
        y += 6.0;
    }
    ui.painter().set(bg_idx, egui::Shape::Vec(shapes));
}

fn stat_card(ui: &mut egui::Ui, w: f32, parts: &[(String, &str)], label: &str) {
    theme::ink_card().inner_margin(Margin { left: 16, right: 16, top: 16, bottom: 14 }).show(ui, |ui| {
        ui.set_width(w - 32.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (v, unit) in parts {
                    ui.label(d(v.clone(), 25.0, theme::TEXT));
                    ui.label(b(*unit, 12.5, Color32::from_rgb(0xD0, 0xD0, 0xD0)));
                    ui.add_space(2.0);
                }
            });
            ui.label(t(label, 14.0, theme::MUTED));
        });
    });
}

fn activity_card(app: &mut DictateApp, ui: &mut egui::Ui, w: f32) {
    let s = &app.stats;
    theme::ink_card().inner_margin(Margin::same(16)).show(ui, |ui| {
        ui.set_width(w - 32.0);
        let col = (w - 32.0) / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (n, label) in [(s.active_days(), "Active days"), (s.current_streak(), "Current streak"), (s.longest_streak(), "Longest streak")] {
                ui.vertical(|ui| {
                    ui.set_width(col);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(d(n.to_string(), 23.0, theme::TEXT));
                        ui.label(b(if n == 1 { "day" } else { "days" }, 12.5, Color32::from_rgb(0xD0, 0xD0, 0xD0)));
                    });
                    ui.label(t(label, 13.0, theme::MUTED));
                });
            }
        });
        ui.add_space(14.0);

        // One column per week, rows Sunday..Saturday, this week on the right.
        let label_w = 16.0;
        let cell = 16.0;
        let avail = w - 32.0 - label_w;
        let weeks = ((avail + 6.0) / (cell + 6.0)).floor().max(4.0) as i64;
        let gapx = (avail - weeks as f32 * cell) / (weeks as f32 - 1.0);
        let today = Local::now().date_naive();
        let this_sunday = today - Duration::days(today.weekday().num_days_from_sunday() as i64);
        let start = this_sunday - Duration::weeks(weeks - 1);
        let first = s.first_day().unwrap_or(today);
        let busiest = s.busiest_day().max(1);
        let (r, _) = ui.allocate_exact_size(vec2(w - 32.0, 7.0 * 22.0 + 14.0), Sense::hover());
        let p = ui.painter();
        for (i, d) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
            p.text(pos2(r.left() + 4.0, r.top() + i as f32 * 22.0 + cell / 2.0), Align2::CENTER_CENTER, d, FontId::proportional(9.0), theme::MUTED);
        }
        let hatch = Stroke::new(1.0, Color32::from_rgb(0x3E, 0x3E, 0x3E));
        let mut last_month = 0;
        for wk in 0..weeks {
            let x = r.left() + label_w + wk as f32 * (cell + gapx);
            for dow in 0..7 {
                let d = start + Duration::days(wk * 7 + dow);
                let cr = Rect::from_min_size(pos2(x, r.top() + dow as f32 * 22.0), vec2(cell, cell));
                if d > today {
                    p.rect_stroke(cr, 3.0, Stroke::new(1.0, Color32::from_rgb(0x44, 0x44, 0x44)), StrokeKind::Inside);
                } else if d < first {
                    // Before the first dictation: striped, like the original.
                    p.rect_filled(cr, 3.0, Color32::from_rgb(0x31, 0x31, 0x31));
                    for k in [4.0, 9.0, 14.0] {
                        p.line_segment([pos2(cr.left() + k - 4.0, cr.bottom() - 1.0), pos2(cr.left() + k + 1.0, cr.top() + 1.0)], hatch);
                    }
                } else {
                    let n = s.count_on(d);
                    let c = if n == 0 { Color32::from_rgb(0x45, 0x45, 0x45) } else { theme::HEAT[((n as f32 / busiest as f32) * 3.99) as usize] };
                    p.rect_filled(cr, 3.0, c);
                }
            }
            let d = start + Duration::days(wk * 7);
            if d.month() != last_month {
                if wk > 0 {
                    p.text(pos2(x + cell / 2.0, r.bottom() - 4.0), Align2::CENTER_CENTER, d.format("%b").to_string(), FontId::proportional(9.0), theme::MUTED);
                }
                last_month = d.month();
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(t("Less", 12.0, theme::MUTED));
            for c in theme::HEAT {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
            }
            ui.label(t("More", 12.0, theme::MUTED));
        });
    });
}

fn personalization_card(app: &mut DictateApp, ui: &mut egui::Ui, w: f32) {
    let words = app.s.dictionary.len();
    theme::ink_card().inner_margin(Margin::same(16)).show(ui, |ui| {
        ui.set_width(w - 32.0);
        ui.set_min_height(210.0);
        let top = ui.min_rect();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            ui.painter().rect_filled(r, 5.0, Color32::from_rgb(0x48, 0x48, 0x48));
            draw_icon(ui.painter(), "spark", r.center(), theme::MUTED);
            ui.label(d(words.to_string(), 25.0, theme::TEXT));
            ui.label(b(if words == 1 { "word" } else { "words" }, 12.5, Color32::from_rgb(0xD0, 0xD0, 0xD0)));
        });
        ui.label(t(format!("Personalization · {} tone", app.s.tone.label()), 14.0, theme::MUTED));
        ui.add_space(10.0);
        let btn = egui::Button::new(m("Personalize", 13.0, theme::TEXT)).fill(Color32::from_rgb(0x48, 0x48, 0x48)).corner_radius(12);
        if ui.add(btn).clicked() {
            app.settings_tab = Some(SettingsTab::Personalization);
        }
        // Radar-style circle with one dot per dictionary word.
        let c = pos2(top.left() + w - 32.0 - 100.0, top.top() + 105.0);
        let p = ui.painter();
        p.circle_filled(c, 100.0, Color32::from_rgb(0x40, 0x40, 0x40));
        for k in 0..5 {
            let a = k as f32 / 5.0 * std::f32::consts::TAU - 1.2;
            p.line_segment([c, c + vec2(a.cos(), a.sin()) * 100.0], Stroke::new(1.0, Color32::from_rgb(0x4A, 0x4A, 0x4A)));
        }
        for (i, wd) in app.s.dictionary.iter().enumerate().take(60) {
            let a = i as f32 * 2.399;
            let rad = 18.0 + (i as f32 * 13.0) % 74.0;
            p.circle_filled(c + vec2(a.cos(), a.sin()) * rad, 2.5, if wd.auto { theme::GREEN } else { theme::HEAT[3] });
        }
        p.circle_filled(c, 6.0, theme::HEAT[3]);
        p.circle_stroke(c, 7.5, Stroke::new(1.0, Color32::from_rgb(0x9C, 0xB4, 0xFF)));
    });
}

fn right_column(app: &mut DictateApp, ui: &mut egui::Ui) {
    theme::soft(theme::CARD_DIM, 14, 2.5, 3).inner_margin(Margin::same(18)).show(ui, |ui| {
        ui.set_width(186.0 - 36.0);
        for a in Action::ALL {
            ui.label(t(a.label(), 12.5, Color32::from_rgb(0xD8, 0xD8, 0xD8)));
            ui.horizontal_wrapped(|ui| {
                let keys = app.s.keys(a);
                if keys.is_empty() {
                    ui.label(t("Not set", 12.0, theme::FAINT));
                }
                for k in keys.iter().take(2) {
                    kbd(ui, k);
                }
            });
            ui.add_space(6.0);
        }
        ui.label(t(if app.s.hold_to_talk { "Hold while you speak" } else { "Press to start and stop" }, 11.5, theme::FAINT));
    });
    ui.add_space(24.0);

    // Start button, for clicking instead of using a shortcut.
    let rec = app.recording.as_ref().map(|(_, a, _)| *a);
    let (label, fill) = match (rec, &app.busy) {
        (Some(_), _) => (format!("{}  Stop & paste", crate::icon::SQUARE), theme::RED),
        (None, Some(_)) => ("Thinking…".to_string(), Color32::from_rgb(0x48, 0x48, 0x48)),
        _ => (format!("{}  Start dictating", crate::icon::MIC), theme::ACCENT),
    };
    let btn = egui::Button::new(b(label, 14.0, theme::ON_FILL)).fill(fill).corner_radius(14).min_size(vec2(186.0, 42.0));
    if ui.add_enabled(app.busy.is_none(), btn).clicked() {
        if rec.is_some() {
            let ctx = ui.ctx().clone();
            app.stop_recording(&ctx);
        } else {
            app.start_recording(Action::Dictate);
        }
    }
    ui.add_space(14.0);

    let (speech_ok, speech) = app.speech_ready();
    let (text_ok, text) = app.text_ready();
    let speech_where = match app.s.speech_engine {
        SpeechEngine::WhisperLocal => "whisper.cpp on this Mac",
        SpeechEngine::OpenAiCompatible => "Your speech server",
    };
    let text_where = match app.s.text_engine {
        TextEngine::Off => "No clean-up",
        TextEngine::Ollama => "Ollama on this Mac",
        TextEngine::OpenAiCompatible => "Your text server",
    };
    for (title, ok, name, place, tint) in [
        ("Speech model", speech_ok, speech, speech_where, Color32::from_rgb(0x3A, 0x47, 0x5C)),
        ("Text model", text_ok, text, if text_ok { text_where } else { "Optional · pastes as heard" }, Color32::from_rgb(0x4A, 0x45, 0x5E)),
    ] {
        let resp = theme::soft(tint, 14, 2.5, 3).inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.set_width(186.0 - 24.0);
            ui.label(b(title, 16.0, theme::TEXT));
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                let missing = if title == "Text model" { theme::AMBER } else { theme::RED };
                ui.painter().circle_filled(r.center(), 3.5, if ok { theme::GREEN } else { missing });
                ui.add(egui::Label::new(b(name, 13.5, theme::TEXT)).truncate());
            });
            ui.label(t(place, 13.0, Color32::from_rgb(0xC8, 0xC8, 0xD0)));
        });
        let click = ui.interact(resp.response.rect, ui.id().with(title), Sense::click());
        if click.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            app.settings_tab = Some(SettingsTab::Models);
            app.refresh_health(ui.ctx());
        }
        ui.add_space(14.0);
    }
    ui.add_space(30.0);
    ui.with_layout(Layout::top_down(Align::Max), |ui| {
        ui.label(t(format!("Version v{VERSION}"), 12.0, theme::MUTED));
        if ui.add(egui::Label::new(t("Check models", 12.0, theme::MUTED).underline()).sense(Sense::click())).clicked() {
            app.refresh_health(ui.ctx());
            app.notify("Checking your models…", false);
        }
    });
}

pub fn kbd(ui: &mut egui::Ui, k: &str) {
    theme::soft(theme::SURFACE, 14, 2.5, 3)
        .corner_radius(5)
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| ui.label(b(pretty_key(k), 13.0, theme::TEXT)));
}

pub fn pretty_key(k: &str) -> String {
    k.split('+')
        .map(|p| match p.trim().to_lowercase().as_str() {
            "alt" | "option" => "⌥".to_string(),
            "shift" => "⇧".to_string(),
            "ctrl" | "control" => "⌃".to_string(),
            "cmd" | "command" | "super" | "meta" => "⌘".to_string(),
            _ => p.trim().to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1000.0 {
        format!("{:.1}K", v / 1000.0)
    } else {
        format!("{v:.0}")
    }
}

fn duration_parts(minutes: f64) -> Vec<(String, &'static str)> {
    let total = minutes.round() as u64;
    let (h, mi) = (total / 60, total % 60);
    if h > 0 { vec![(h.to_string(), "hr"), (mi.to_string(), "min")] } else { vec![(mi.to_string(), "min")] }
}

fn duration_text(minutes: f64) -> String {
    duration_parts(minutes).iter().map(|(v, u)| format!("{v} {u}")).collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// History

pub fn pill_tabs<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, items: &[(T, &str)]) {
    theme::soft(theme::SURFACE, 18, 2.5, 3).inner_margin(Margin::same(4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (v, label) in items {
                let active = *current == *v;
                let btn = egui::Button::new(m(*label, 14.0, if active { theme::TEXT } else { Color32::from_rgb(0xC8, 0xC8, 0xC8) }))
                    .fill(if active { Color32::from_rgb(0x1E, 0x1E, 0x1E) } else { Color32::TRANSPARENT })
                    .corner_radius(14)
                    .min_size(vec2(0.0, 28.0));
                if ui.add(btn).clicked() {
                    *current = *v;
                }
            }
        });
    });
}

enum RowAction {
    Retry,
    Delete,
    Details,
    SaveEdit,
}

fn history(app: &mut DictateApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(b("History", 38.0, theme::TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.menu_button(t(crate::icon::ELLIPSIS, 18.0, theme::MUTED), |ui| {
                if ui.button("Copy all as text").clicked() {
                    let all = app.history.iter().rev().filter(|e| e.error.is_none()).map(|e| e.text.clone()).collect::<Vec<_>>().join("\n\n");
                    ui.ctx().copy_text(all);
                    ui.close();
                }
                if ui.button(t("Clear all history", 14.0, theme::RED)).clicked() {
                    app.clear_history();
                    ui.close();
                }
            });
            ui.label(t(format!("{} Stored only on this Mac", crate::icon::LOCK), 14.0, Color32::from_rgb(0xD0, 0xD0, 0xD0)));
        });
    });
    ui.add_space(10.0);
    theme::soft(theme::PANEL, 10, 2.0, 2).inner_margin(Margin::symmetric(12, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(m(format!("{} Keep history", crate::icon::ARCHIVE), 14.0, theme::TEXT));
                ui.label(t("Choose how long to keep your history on this device.", 14.0, Color32::from_rgb(0xC8, 0xC8, 0xC8)));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut k = app.s.keep_history;
                egui::ComboBox::from_id_salt("keep").width(140.0).selected_text(m(k.label(), 14.0, theme::TEXT)).show_ui(ui, |ui| {
                    for o in KeepHistory::ALL {
                        ui.selectable_value(&mut k, o, o.label());
                    }
                });
                if k != app.s.keep_history {
                    app.set_keep_history(k);
                }
            });
        });
        ui.add_space(6.0);
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(r, 0.0, theme::LINE);
        ui.add_space(6.0);
        ui.label(m(format!("{} Your data stays yours", crate::icon::LOCK), 14.0, theme::TEXT));
        ui.label(para(
            "Recordings and text are kept only on this Mac. Your voice goes only to the speech and text models you chose in Models.",
            14.0,
            Color32::from_rgb(0xC8, 0xC8, 0xC8),
        ));
    });
    ui.add_space(14.0);
    pill_tabs(
        ui,
        &mut app.hist_filter,
        &[(None, "All"), (Some(Action::Dictate), "Dictations"), (Some(Action::Translate), "Translations"), (Some(Action::Ask), "Ask anything")],
    );
    ui.add_space(14.0);

    let items: Vec<_> = app.history.iter().rev().filter(|e| app.hist_filter.is_none_or(|f| f == e.action)).cloned().collect();
    if items.is_empty() {
        ui.add_space(30.0);
        let key = app.s.dictate_keys.first().map(|k| pretty_key(k)).unwrap_or_else(|| "your shortcut".into());
        ui.vertical_centered(|ui| {
            ui.label(m("Nothing here yet", 18.0, theme::TEXT));
            ui.label(t(format!("Press {key} anywhere, speak, and your words show up here."), 14.0, theme::MUTED));
        });
        return;
    }

    let ctx = ui.ctx().clone();
    let mut action: Option<(RowAction, u64)> = None;
    let mut i = 0;
    while i < items.len() {
        let day = stats::local_date(items[i].when);
        ui.label(t(stats::day_heading(day), 13.0, Color32::from_rgb(0xC8, 0xC8, 0xC8)));
        ui.add_space(2.0);
        theme::soft(theme::SURFACE, 12, 2.0, 2).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let mut first = true;
            while i < items.len() && stats::local_date(items[i].when) == day {
                let e = &items[i];
                if !first {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0x30, 0x30, 0x30));
                }
                first = false;
                if let Some(a) = history_row(app, ui, e) {
                    action = Some((a, e.when));
                }
                i += 1;
            }
        });
        ui.add_space(12.0);
    }
    match action {
        Some((RowAction::Retry, w)) => app.retry(&ctx, w),
        Some((RowAction::Delete, w)) => app.delete_entry(w),
        Some((RowAction::Details, w)) => app.expanded = if app.expanded == Some(w) { None } else { Some(w) },
        Some((RowAction::SaveEdit, w)) => {
            if let Some((_, text)) = app.editing.take() {
                app.save_edit(w, text);
            }
        }
        None => {}
    }
}

fn history_row(app: &mut DictateApp, ui: &mut egui::Ui, e: &crate::settings::Entry) -> Option<RowAction> {
    let mut out = None;
    egui::Frame::new().inner_margin(Margin { left: 12, right: 12, top: 10, bottom: 10 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.add_sized(vec2(58.0, 18.0), egui::Label::new(t(stats::clock(e.when), 12.5, theme::MUTED)));
            let right_w = 150.0;
            let text_w = ui.available_width() - right_w;
            ui.vertical(|ui| {
                ui.set_width(text_w);
                if let Some(err) = &e.error {
                    ui.label(t(format!("{err}  {}", crate::icon::INFO), 14.0, Color32::from_rgb(0xB8, 0xB8, 0xB8)))
                        .on_hover_text(format!("{} · {:.1}s recording", e.action.label(), e.seconds));
                } else if let Some((_, text)) = app.editing.as_mut().filter(|(w, _)| *w == e.when) {
                    ui.add(egui::TextEdit::multiline(text).desired_width(text_w).desired_rows(3));
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            out = Some(RowAction::SaveEdit);
                        }
                        if ui.button("Cancel").clicked() {
                            app.editing = None;
                        }
                    });
                } else {
                    match e.action {
                        Action::Dictate => {}
                        Action::Translate => {
                            ui.label(t("Translated", 11.5, theme::HEAT[3]));
                        }
                        Action::Ask => {
                            ui.label(t("Ask anything", 11.5, theme::HEAT[3]));
                        }
                    }
                    ui.label(para(&e.text, 14.0, theme::TEXT));
                    if app.expanded == Some(e.when) {
                        ui.add_space(4.0);
                        if !e.selection.is_empty() {
                            ui.label(t(format!("Selected text: {}", e.selection), 12.5, theme::MUTED));
                        }
                        ui.label(t(format!("Heard: {}", e.raw), 12.5, theme::MUTED));
                        ui.label(t(format!("{:.1}s · speech: {} · text: {}", e.seconds, e.speech_model, e.text_model), 12.0, theme::FAINT));
                    }
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                ui.menu_button(t(crate::icon::ELLIPSIS, 15.0, theme::MUTED), |ui| {
                    if e.error.is_none() {
                        if ui.button("Copy").clicked() {
                            ui.ctx().copy_text(e.text.clone());
                            ui.close();
                        }
                        if ui.button("Copy what you said").clicked() {
                            ui.ctx().copy_text(e.raw.clone());
                            ui.close();
                        }
                        if ui.button("Correct text…").clicked() {
                            app.editing = Some((e.when, e.text.clone()));
                            ui.close();
                        }
                        if ui.button(if app.expanded == Some(e.when) { "Hide details" } else { "Show details" }).clicked() {
                            out = Some(RowAction::Details);
                            ui.close();
                        }
                    }
                    if ui.button(t("Delete", 14.0, theme::RED)).clicked() {
                        out = Some(RowAction::Delete);
                        ui.close();
                    }
                });
                if e.error.is_some() && e.audio.is_some() {
                    let btn = egui::Button::new(m(format!("{} Retry", crate::icon::ROTATE), 14.0, theme::TEXT)).fill(Color32::from_rgb(0x44, 0x44, 0x44)).corner_radius(6);
                    if ui.add_enabled(app.busy.is_none(), btn).clicked() {
                        out = Some(RowAction::Retry);
                    }
                } else if e.error.is_none() && ui.add(egui::Button::new(t("Copy", 13.0, theme::MUTED)).frame(false)).clicked() {
                    ui.ctx().copy_text(e.text.clone());
                    app.notify("Copied", false);
                }
            });
        });
    });
    out
}

// ---------------------------------------------------------------------------
// Dictionary

fn dictionary(app: &mut DictateApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(b("Dictionary", 38.0, theme::TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let btn = egui::Button::new(m("New word", 14.0, Color32::from_rgb(0x1A, 0x1A, 0x1A)))
                .fill(Color32::from_rgb(0xF4, 0xF4, 0xF4))
                .corner_radius(16)
                .min_size(vec2(0.0, 32.0));
            if ui.add(btn).clicked() {
                app.new_word = Some(String::new());
            }
        });
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (auto_tab, manual_tab) = (
            format!("{}  Auto-added", crate::icon::SPARKLES),
            format!("{}  Manually-added", crate::icon::PENCIL),
        );
        pill_tabs(ui, &mut app.dict_filter, &[(0, "All"), (1, &auto_tab), (2, &manual_tab)]);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
            ui.painter().circle_filled(r.center(), 16.0, Color32::from_rgb(0x3C, 0x3C, 0x3C));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, crate::icon::SEARCH, FontId::proportional(13.0), theme::MUTED);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                app.dict_search = if app.dict_search.is_some() { None } else { Some(String::new()) };
            }
            if let Some(q) = &mut app.dict_search {
                ui.add(egui::TextEdit::singleline(q).hint_text("Search words").desired_width(220.0));
            }
        });
    });
    ui.add_space(10.0);

    if let Some(w) = &mut app.new_word {
        let mut add = false;
        let mut cancel = false;
        theme::soft(theme::PANEL, 10, 2.0, 2).inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(t("Add a name, term or spelling you want written exactly like this.", 13.5, theme::MUTED));
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(w).hint_text("e.g. Kubernetes, Adaeze, feat/idle-timeout").desired_width(360.0));
                if !r.has_focus() && !r.lost_focus() && w.is_empty() {
                    r.request_focus();
                }
                add = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                add |= ui.add(egui::Button::new(b("Add", 14.0, theme::ON_FILL)).fill(theme::ACCENT)).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        ui.add_space(10.0);
        if add {
            let word = w.trim().to_string();
            if !word.is_empty() && !app.s.dictionary.iter().any(|d| d.word == word) {
                app.s.dictionary.push(DictWord { word, auto: false });
                app.changed();
            }
            app.new_word = None;
        } else if cancel {
            app.new_word = None;
        }
    }

    let q = app.dict_search.clone().unwrap_or_default().to_lowercase();
    let words: Vec<(usize, DictWord)> = app
        .s
        .dictionary
        .iter()
        .cloned()
        .enumerate()
        .rev()
        .filter(|(_, d)| match app.dict_filter {
            1 => d.auto,
            2 => !d.auto,
            _ => true,
        })
        .filter(|(_, d)| q.is_empty() || d.word.to_lowercase().contains(&q))
        .collect();
    if words.is_empty() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.label(m("No words yet", 18.0, theme::TEXT));
            ui.label(para("Add names and terms here, or correct a dictation in History and new words are learned automatically.", 14.0, theme::MUTED));
        });
        return;
    }
    let cols = 3;
    let gap = 8.0;
    let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let mut remove = None;
    for row in words.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (idx, d) in row {
                let (r, resp) = ui.allocate_exact_size(vec2(w, 38.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(r, 6.0, Color32::from_rgb(0x26, 0x26, 0x26));
                p.rect_stroke(r, 6.0, Stroke::new(1.0, Color32::from_rgb(0x33, 0x33, 0x33)), StrokeKind::Inside);
                let (icon, c) = if d.auto { (crate::icon::SPARKLES, theme::GREEN) } else { (crate::icon::PENCIL, theme::FAINT) };
                p.text(pos2(r.left() + 18.0, r.center().y), Align2::CENTER_CENTER, icon, FontId::proportional(11.0), c);
                p.text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, &d.word, FontId::new(14.0, theme::medium()), theme::TEXT);
                if resp.hovered() {
                    let x = Rect::from_center_size(pos2(r.right() - 18.0, r.center().y), Vec2::splat(20.0));
                    let xr = ui.interact(x, ui.id().with(("del", *idx)), Sense::click());
                    draw_icon(ui.painter(), "close", x.center(), if xr.hovered() { theme::TEXT } else { theme::MUTED });
                    if xr.clicked() {
                        remove = Some(*idx);
                    }
                }
            }
        });
        ui.add_space(4.0);
    }
    if let Some(i) = remove {
        app.s.dictionary.remove(i);
        app.changed();
    }
}

fn toast(app: &mut DictateApp, ctx: &egui::Context) {
    let Some(toast) = &app.toast else { return };
    egui::Area::new(egui::Id::new("toast")).order(egui::Order::Tooltip).anchor(Align2::CENTER_BOTTOM, vec2(108.0, -28.0)).show(ctx, |ui| {
        egui::Frame::new()
            .fill(if toast.error { theme::RED } else { theme::SURFACE })
            .stroke(Stroke::new(2.5, theme::INK))
            .corner_radius(10)
            .inner_margin(Margin::symmetric(16, 10))
            .show(ui, |ui| {
                ui.set_max_width(520.0);
                ui.label(t(&toast.text, 14.0, theme::TEXT));
            });
    });
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
}
