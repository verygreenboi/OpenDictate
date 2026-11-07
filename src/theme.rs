//! Dark look matching the reference screenshots.

use eframe::egui::{self, Color32, FontFamily, FontId, RichText};

pub const BG: Color32 = Color32::from_rgb(0x23, 0x23, 0x23);
pub const SIDEBAR: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x2C);
pub const NAV_ACTIVE: Color32 = Color32::from_rgb(0x3B, 0x3B, 0x3B);
pub const CARD: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
pub const CARD_DIM: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
pub const PANEL: Color32 = Color32::from_rgb(0x29, 0x29, 0x29);
pub const LINE: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF2);
pub const MUTED: Color32 = Color32::from_rgb(0xA6, 0xA6, 0xA6);
pub const FAINT: Color32 = Color32::from_rgb(0x78, 0x78, 0x78);
pub const ACCENT: Color32 = Color32::from_rgb(0x4F, 0x7C, 0xF5);
pub const GREEN: Color32 = Color32::from_rgb(0x2F, 0xA8, 0x6B);
pub const RED: Color32 = Color32::from_rgb(0xE5, 0x4B, 0x5A);
pub const AMBER: Color32 = Color32::from_rgb(0xE8, 0xA8, 0x38);
pub const HEAT: [Color32; 4] = [
    Color32::from_rgb(0x2A, 0x3A, 0x6E),
    Color32::from_rgb(0x34, 0x4D, 0xA6),
    Color32::from_rgb(0x44, 0x63, 0xD6),
    Color32::from_rgb(0x6D, 0x8F, 0xFA),
];

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
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

/// Uses the system's San Francisco font at three weights when it's there,
/// falling back to egui's built-in font elsewhere.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    let sf = std::fs::read("/System/Library/Fonts/SFNS.ttf").ok();
    // Lucide sits behind every family, so an icon codepoint inside an ordinary
    // string just renders. Its glyphs live in a private use area, which no text
    // font claims, so nothing else is affected.
    fonts.font_data.insert(
        "lucide".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/lucide.ttf"))),
    );
    let mut add = |name: &str, family: FontFamily, weight: f32| {
        let mut list = Vec::new();
        if let Some(bytes) = &sf {
            let mut tweak = egui::FontTweak::default();
            tweak.coords = egui::epaint::text::VariationCoords::new([("wght", weight)]);
            fonts.font_data.insert(
                name.to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes.clone()).tweak(tweak)),
            );
            list.push(name.to_owned());
        }
        list.push("lucide".to_owned());
        list.extend(fallback.iter().cloned());
        fonts.families.insert(family, list);
    };
    add("sf-regular", FontFamily::Proportional, 420.0);
    add("sf-medium", medium(), 560.0);
    add("sf-bold", bold(), 700.0);
    ctx.set_fonts(fonts);

    let mut style = egui::Style::default();
    style.visuals = egui::Visuals::dark();
    let v = &mut style.visuals;
    v.panel_fill = BG;
    v.window_fill = Color32::from_rgb(0x34, 0x34, 0x34);
    v.extreme_bg_color = Color32::from_rgb(0x1E, 0x1E, 0x1E);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.hyperlink_color = ACCENT;
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x44, 0x44, 0x44);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x44, 0x44, 0x44);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x50, 0x50, 0x50);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x5A, 0x5A, 0x5A);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive] {
        w.corner_radius = egui::CornerRadius::same(8);
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LINE);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
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
