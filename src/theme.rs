//! Dark look matching the reference screenshots.

use eframe::egui::{self, Color32, FontFamily, FontId, RichText};

// Paper & Ink, dark variant. Token names follow the kit's CSS so the two can
// be compared line by line; the older names are kept so call sites still read
// the same.
pub const PAPER: Color32 = Color32::from_rgb(0x14, 0x14, 0x0F);
pub const INK: Color32 = Color32::from_rgb(0xF4, 0xF3, 0xEF);
pub const SURFACE: Color32 = Color32::from_rgb(0x1F, 0x1E, 0x18);

pub const BG: Color32 = PAPER;
pub const SIDEBAR: Color32 = SURFACE;
pub const NAV_ACTIVE: Color32 = INK;
pub const CARD: Color32 = SURFACE;
pub const CARD_DIM: Color32 = PAPER;
pub const PANEL: Color32 = SURFACE;
/// --hairline
pub const LINE: Color32 = Color32::from_rgba_premultiplied(0x22, 0x22, 0x1E, 0xFF);
pub const TEXT: Color32 = INK;
/// Ink stepped back, for secondary copy.
pub const MUTED: Color32 = Color32::from_rgb(0xA8, 0xA7, 0xA2);
// Lifted off the kit's value: at #797873 this was 3.78:1 on surface, and it
// carries small print, which needs 4.5.
pub const FAINT: Color32 = Color32::from_rgb(0x8B, 0x8A, 0x85);
/// --follow. The kit's blue reads as settled and working.
pub const ACCENT: Color32 = Color32::from_rgb(0x8F, 0xB6, 0xEF);
/// --convert, used where the old theme said green: a thing that is fine.
pub const GREEN: Color32 = Color32::from_rgb(0x9A, 0xBE, 0xEF);
/// --red
pub const RED: Color32 = Color32::from_rgb(0xE0, 0x49, 0x3D);
/// --y2. Yellow has noticed; something wants attention soon.
pub const AMBER: Color32 = Color32::from_rgb(0xF2, 0xC3, 0x1A);
/// --y1 and --y3, for the softer and heavier ends of yellow.
pub const Y1: Color32 = Color32::from_rgb(0xF8, 0xE0, 0x8A);
pub const Y3: Color32 = Color32::from_rgb(0xC9, 0x8F, 0x06);
/// What sits on top of a light fill: ink, accent, red or yellow all take
/// paper, as the kit's --on-red and --on-yellow do. White on the kit's blue is
/// about 2.1:1, which is why it has no such token.
pub const ON_FILL: Color32 = PAPER;
/// Paper stepped back, for secondary copy on a light fill.
pub const ON_FILL_DIM: Color32 = Color32::from_rgb(0x4A, 0x49, 0x44);

/// --soft-shadow. Still hard edged, but translucent black rather than ink.
/// The kit reserves the full ink shadow for things you press; content cards
/// get this, which is why they do not shout.
pub const SOFT_SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 115);
pub const HEAT: [Color32; 4] = [
    Color32::from_rgb(0x2B, 0x3A, 0x55),
    Color32::from_rgb(0x44, 0x63, 0x96),
    Color32::from_rgb(0x6D, 0x90, 0xCF),
    Color32::from_rgb(0x9A, 0xBE, 0xEF),
];
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}
/// Marsipan, the kit's rounded display face. Headings and figures only.
pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}

/// A surface in the kit's hand: thick outline and a hard offset shadow.
///
/// The shadow is never blurred - blur 0 is what makes it read as ink on paper
/// rather than a drop shadow. Its colour is the choice that matters: ink for
/// something you press, soft for something you only read.
pub fn surface(fill: Color32, radius: u8, border: f32, lift: i8, shadow: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(border, INK))
        .corner_radius(egui::CornerRadius::same(radius))
        .shadow(egui::epaint::Shadow { offset: [lift, lift + 1], blur: 0, spread: 0, color: shadow })
}
/// Content you read. Matches the kit's .card: 2.5px outline, 6/7 soft shadow.
pub fn ink_card() -> egui::Frame {
    surface(SURFACE, 15, 2.5, 6, SOFT_SHADOW)
}
/// A larger reading surface. Matches .step-card and .menu: 3px, 7/8 soft.
pub fn ink_panel() -> egui::Frame {
    surface(SURFACE, 18, 3.0, 7, SOFT_SHADOW)
}
/// Another surface you only read, at a size of its own choosing.
pub fn soft(fill: Color32, radius: u8, border: f32, lift: i8) -> egui::Frame {
    surface(fill, radius, border, lift, SOFT_SHADOW)
}
/// Something you press. Keeps the full ink shadow, as .inspect does.
pub fn ink(fill: Color32, radius: u8, border: f32, lift: i8) -> egui::Frame {
    surface(fill, radius, border, lift, INK)
}

pub fn t(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).size(size).color(color)
}
pub fn b(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(FontId::new(size, bold())).color(color)
}
pub fn m(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(FontId::new(size, medium())).color(color)
}

/// Display type: Marsipan, for titles and figures. The kit uses it at 40 for a
/// big title, 27 for a figure and 15 on buttons; body copy stays Nunito.
pub fn d(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(FontId::new(size, display())).color(color)
}

/// Body copy that runs to more than one line.
///
/// egui spaces rows by the font's own metrics, which is about 1.2x the size:
/// fine for a label, too tight to read a paragraph in. Anything that wraps
/// should use this instead of `t`.
pub fn para(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    t(s, size, color).line_height(Some(size * 1.5))
}

/// Paper & Ink's faces: Marsipan for display, Nunito for everything else.
///
/// Nunito is a variable font, so the kit's three body weights come from one
/// file at different points on its wght axis. Lucide sits behind all of them
/// so icon codepoints keep rendering inside ordinary strings.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts.font_data.insert(
        "lucide".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/lucide.ttf"))),
    );
    fonts.font_data.insert(
        "marsipan".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/Marsipan-Regular.ttf"))),
    );
    let nunito = include_bytes!("../assets/Nunito-Variable.ttf");
    let mut add = |name: &str, family: FontFamily, weight: f32| {
        let mut tweak = egui::FontTweak::default();
        tweak.coords = egui::epaint::text::VariationCoords::new([("wght", weight)]);
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(nunito).tweak(tweak)),
        );
        let mut list = vec![name.to_owned(), "lucide".to_owned()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(family, list);
    };
    // The kit's body weights: 600 copy, 700 emphasis, 800 buttons and labels.
    add("nunito-600", FontFamily::Proportional, 600.0);
    add("nunito-700", medium(), 700.0);
    add("nunito-800", bold(), 800.0);
    let mut disp = vec!["marsipan".to_owned(), "nunito-800".to_owned(), "lucide".to_owned()];
    disp.extend(fallback.iter().cloned());
    fonts.families.insert(display(), disp);
    ctx.set_fonts(fonts);

    let mut style = egui::Style::default();
    style.visuals = egui::Visuals::dark();
    let v = &mut style.visuals;
    v.panel_fill = BG;
    v.window_fill = Color32::from_rgb(0x34, 0x34, 0x34);
    v.extreme_bg_color = PAPER;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.hyperlink_color = ACCENT;
    // Controls carry the outline too. The kit presses a button by moving it
    // onto its own shadow, so pressed state is the ink fill with paper on top.
    v.widgets.inactive.weak_bg_fill = SURFACE;
    v.widgets.inactive.bg_fill = SURFACE;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(2.5, INK);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, INK);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2B, 0x2A, 0x23);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x2B, 0x2A, 0x23);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(2.5, INK);
    v.widgets.active.weak_bg_fill = INK;
    v.widgets.active.bg_fill = INK;
    v.widgets.active.bg_stroke = egui::Stroke::new(2.5, INK);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, PAPER);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive] {
        w.corner_radius = egui::CornerRadius::same(11);
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LINE);

    // Popups and tooltips follow .menu: an ink edge over a hard shadow. The
    // kit's .tip is yellow, but egui draws every popup from one set of values,
    // so a yellow tooltip would drag the dropdown lists with it.
    v.window_fill = SURFACE;
    v.window_stroke = egui::Stroke::new(2.5, INK);
    v.window_corner_radius = egui::CornerRadius::same(14);
    v.menu_corner_radius = egui::CornerRadius::same(14);
    let hard = egui::epaint::Shadow { offset: [3, 4], blur: 0, spread: 0, color: SOFT_SHADOW };
    v.popup_shadow = hard;
    v.window_shadow = hard;

    // Scrollbar: a slim ink handle that keeps out of the way until wanted.
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_width = 9.0;
    style.spacing.scroll.floating_width = 5.0;
    style.spacing.scroll.foreground_color = true;
    style.spacing.scroll.dormant_handle_opacity = 0.25;
    style.spacing.scroll.active_handle_opacity = 0.6;
    style.spacing.scroll.interact_handle_opacity = 0.9;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.active_background_opacity = 0.0;
    // The kit's slider is a plain track with an ink thumb; egui's trailing
    // fill would paint it in the selection colour, which is the accent blue.
    style.visuals.slider_trailing_fill = false;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(13.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    for (ts, size) in [
        (egui::TextStyle::Body, 14.0),
        (egui::TextStyle::Button, 14.0),
        (egui::TextStyle::Small, 12.0),
        (egui::TextStyle::Heading, 22.0),
    ] {
        if let Some(f) = style.text_styles.get_mut(&ts) {
            f.size = size;
        }
    }
    ctx.set_global_style(style);
}
