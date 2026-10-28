//! Design tokens. Colours follow the system light/dark theme (eframe's
//! default `ThemePreference::System`); an explicit user override is a later,
//! separately reviewed settings slice.
use egui::Color32;

#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub radius: f32,
    pub viewport_background: Color32,
    pub chrome: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub tag_text: Color32,
    pub tag_fill: Color32,
    pub warning_text: Color32,
    pub warning_fill: Color32,
}
const DARK: Tokens = Tokens {
    radius: 6.0,
    viewport_background: Color32::from_rgb(24, 27, 32),
    chrome: Color32::from_rgb(30, 33, 38),
    muted: Color32::from_rgb(150, 156, 166),
    accent: Color32::from_rgb(94, 156, 255),
    tag_text: Color32::from_rgb(214, 190, 255),
    tag_fill: Color32::from_rgb(52, 42, 74),
    warning_text: Color32::from_rgb(255, 214, 153),
    warning_fill: Color32::from_rgb(82, 52, 18),
};
const LIGHT: Tokens = Tokens {
    radius: 6.0,
    viewport_background: Color32::from_rgb(236, 239, 243),
    chrome: Color32::from_rgb(246, 247, 249),
    muted: Color32::from_rgb(92, 99, 110),
    accent: Color32::from_rgb(28, 99, 214),
    tag_text: Color32::from_rgb(88, 46, 160),
    tag_fill: Color32::from_rgb(236, 228, 252),
    warning_text: Color32::from_rgb(110, 60, 0),
    warning_fill: Color32::from_rgb(255, 238, 204),
};
impl Tokens {
    pub fn of(ui: &egui::Ui) -> Self {
        if ui.visuals().dark_mode { DARK } else { LIGHT }
    }
}

/// Spacing and type scale shared by both themes.
pub fn install(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        for (text, size) in [
            (egui::TextStyle::Body, 14.0),
            (egui::TextStyle::Button, 14.0),
            (egui::TextStyle::Small, 12.0),
            (egui::TextStyle::Heading, 18.0),
            (egui::TextStyle::Monospace, 13.0),
        ] {
            if let Some(font) = style.text_styles.get_mut(&text) {
                font.size = size;
            }
        }
    });
}

/// A small rounded tag, e.g. "Synthetic demo".
pub fn tag(ui: &mut egui::Ui, text: &str, hover: &str) -> egui::Response {
    let tokens = Tokens::of(ui);
    egui::Frame::new()
        .fill(tokens.tag_fill)
        .corner_radius(tokens.radius)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).small().color(tokens.tag_text))
        })
        .inner
        .on_hover_text(hover)
}
