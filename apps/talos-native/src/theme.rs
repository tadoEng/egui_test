//! Design tokens, type and icons (APP-01A.2, after the #52 design target).
//!
//! Colours follow the system light/dark theme (eframe's default
//! `ThemePreference::System`); an explicit user override is a later,
//! separately reviewed settings slice. Type is Inter (SIL OFL 1.1), one
//! variable font registered at three weights. Icons are a Lucide subset
//! (ISC), rasterized to masks and tinted at runtime.
use egui::{Color32, FontFamily, FontId, RichText};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub radius: f32,
    pub card_radius: f32,
    pub chrome: Color32,
    pub chrome_line: Color32,
    pub page: Color32,
    pub card: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub shadow: egui::Shadow,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_weak: Color32,
    pub synthetic: Color32,
    pub synthetic_weak: Color32,
    pub exact: Color32,
    pub resolved: Color32,
    pub approx: Color32,
    pub approx_weak: Color32,
    pub warning_text: Color32,
    pub warning_fill: Color32,
    /// The real viewport background (`lab_viewport::BACKGROUND`).
    pub viewport_background: Color32,
}
const DARK: Tokens = Tokens {
    radius: 6.0,
    card_radius: 12.0,
    chrome: Color32::from_rgb(18, 22, 28),
    chrome_line: Color32::from_rgb(34, 39, 47),
    page: Color32::from_rgb(15, 19, 24),
    card: Color32::from_rgb(22, 27, 34),
    surface: Color32::from_rgba_premultiplied(21, 26, 33, 242),
    border: Color32::from_rgb(40, 46, 55),
    shadow: egui::Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(115),
    },
    text: Color32::from_rgb(232, 235, 239),
    muted: Color32::from_rgb(141, 150, 165),
    faint: Color32::from_rgb(95, 104, 118),
    accent: Color32::from_rgb(106, 165, 255),
    accent_weak: Color32::from_rgba_premultiplied(15, 23, 36, 36),
    synthetic: Color32::from_rgb(195, 177, 255),
    synthetic_weak: Color32::from_rgb(40, 34, 64),
    exact: Color32::from_rgb(79, 197, 143),
    resolved: Color32::from_rgb(106, 165, 255),
    approx: Color32::from_rgb(240, 178, 87),
    approx_weak: Color32::from_rgb(52, 40, 22),
    warning_text: Color32::from_rgb(255, 214, 153),
    warning_fill: Color32::from_rgb(82, 52, 18),
    viewport_background: lab_viewport::BACKGROUND,
};
const LIGHT: Tokens = Tokens {
    radius: 6.0,
    card_radius: 12.0,
    chrome: Color32::from_rgb(247, 248, 250),
    chrome_line: Color32::from_rgb(228, 231, 236),
    page: Color32::from_rgb(238, 241, 245),
    card: Color32::WHITE,
    surface: Color32::from_rgba_premultiplied(245, 245, 245, 245),
    border: Color32::from_rgb(225, 229, 235),
    shadow: egui::Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(40),
    },
    text: Color32::from_rgb(23, 27, 33),
    muted: Color32::from_rgb(93, 102, 117),
    faint: Color32::from_rgb(139, 147, 161),
    accent: Color32::from_rgb(45, 111, 224),
    accent_weak: Color32::from_rgb(229, 238, 252),
    synthetic: Color32::from_rgb(107, 69, 200),
    synthetic_weak: Color32::from_rgb(240, 235, 252),
    exact: Color32::from_rgb(31, 143, 92),
    resolved: Color32::from_rgb(45, 111, 224),
    approx: Color32::from_rgb(178, 106, 0),
    approx_weak: Color32::from_rgb(253, 243, 226),
    warning_text: Color32::from_rgb(110, 60, 0),
    warning_fill: Color32::from_rgb(255, 238, 204),
    viewport_background: lab_viewport::BACKGROUND,
};
impl Tokens {
    pub fn of(ui: &egui::Ui) -> Self {
        Self::dark(ui.visuals().dark_mode)
    }
    pub fn dark(dark: bool) -> Self {
        if dark { DARK } else { LIGHT }
    }
}

/// The 12/13/15/18 scale: body text is 13 pt, and nothing (engineering
/// values, warnings, section labels) drops below 12.
pub const BODY: f32 = 13.0;
pub const SMALL: f32 = 12.0;
pub const TITLE: f32 = 15.0;
pub const NAME: f32 = 18.0;

pub fn medium() -> FontFamily {
    FontFamily::Name("inter-medium".into())
}
pub fn semibold() -> FontFamily {
    FontFamily::Name("inter-semibold".into())
}

/// Inter at weights 400 (proportional default), 500 and 600, plus egui's
/// default fallbacks; spacing and the type scale for both themes.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let bytes: &'static [u8] = include_bytes!("../assets/fonts/Inter-Variable.ttf");
    for (name, weight) in [
        ("inter-400", 400.0),
        ("inter-500", 500.0),
        ("inter-600", 600.0),
    ] {
        fonts.font_data.insert(
            name.to_owned(),
            Arc::new(egui::FontData::from_static(bytes).tweak(egui::FontTweak {
                coords: egui::epaint::text::VariationCoords::new([("wght", weight)]),
                ..Default::default()
            })),
        );
    }
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let family = |primary: &str| {
        std::iter::once(primary.to_owned())
            .chain(fallbacks.iter().cloned())
            .collect::<Vec<_>>()
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family("inter-400"));
    fonts.families.insert(medium(), family("inter-500"));
    fonts.families.insert(semibold(), family("inter-600"));
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        for (text, size, family) in [
            (egui::TextStyle::Body, BODY, FontFamily::Proportional),
            (egui::TextStyle::Button, BODY, medium()),
            (egui::TextStyle::Small, SMALL, FontFamily::Proportional),
            (egui::TextStyle::Heading, NAME, semibold()),
            (egui::TextStyle::Monospace, 13.0, FontFamily::Monospace),
        ] {
            style.text_styles.insert(text, FontId::new(size, family));
        }
    });
}

/// A label in the uppercase section style (`SECTION`), 12 pt, muted.
pub fn section(ui: &mut egui::Ui, text: &str) {
    let tokens = Tokens::of(ui);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(SMALL)
            .family(semibold())
            .extra_letter_spacing(0.6)
            .color(tokens.muted),
    );
}

/// A small rounded pill, e.g. "Synthetic". Sized from its text alone, so a
/// full-height row cannot stretch it.
pub fn pill(
    ui: &mut egui::Ui,
    text: &str,
    fg: Color32,
    bg: Color32,
    hover: &str,
) -> egui::Response {
    let galley = egui::WidgetText::from(RichText::new(text).size(SMALL).family(medium()))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Small,
        );
    let size = galley.size() + egui::vec2(18.0, 6.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, text));
    ui.painter().rect_filled(rect, rect.height() / 2.0, bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
    response.on_hover_text(hover)
}

/// A chip with a coloured dot and text, so state is never colour-only. One
/// allocation, so a wrapping row moves the whole chip to the next line.
pub fn chip(ui: &mut egui::Ui, dot: Option<Color32>, text: &str) -> egui::Response {
    let tokens = Tokens::of(ui);
    let galley = egui::WidgetText::from(RichText::new(text).size(SMALL).family(medium()))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Small,
        );
    let lead = if dot.is_some() { 13.0 } else { 0.0 };
    let size = galley.size() + egui::vec2(18.0 + lead, 8.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, text));
    let painter = ui.painter();
    painter.rect(
        rect,
        rect.height() / 2.0,
        tokens.card,
        egui::Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );
    if let Some(color) = dot {
        painter.circle_filled(
            egui::pos2(rect.left() + 9.0 + 3.5, rect.center().y),
            3.5,
            color,
        );
    }
    let text_at = egui::pos2(
        rect.left() + 9.0 + lead,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_at, galley, tokens.text);
    response
}

/// The canvas-card frame shared by floating panels, toolbars and the
/// version strip.
pub fn card(ui: &egui::Ui) -> egui::Frame {
    let tokens = Tokens::of(ui);
    egui::Frame::new()
        .fill(tokens.surface)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(tokens.card_radius)
        .shadow(tokens.shadow)
}
