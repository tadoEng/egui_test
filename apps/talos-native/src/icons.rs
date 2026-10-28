//! A small Lucide icon subset (ISC; `assets/icons/LICENSE-lucide.txt`),
//! embedded as 48 px white masks and tinted when drawn. Icon-only buttons
//! always carry an accessible name and a tooltip.
use egui::{Color32, TextureHandle, TextureOptions};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Icon {
    Model,
    Results,
    Compare,
    History,
    Reports,
    ChevronDown,
    Lock,
    Unplug,
    Search,
    PanelLeft,
    PanelRight,
    Fit,
    Close,
    Layers,
    More,
    Commit,
    Info,
    Copy,
    Floating,
    Warning,
}
impl Icon {
    fn png(self) -> &'static [u8] {
        match self {
            Self::Model => include_bytes!("../assets/icons/box.png"),
            Self::Results => include_bytes!("../assets/icons/chart-line.png"),
            Self::Compare => include_bytes!("../assets/icons/git-compare.png"),
            Self::History => include_bytes!("../assets/icons/history.png"),
            Self::Reports => include_bytes!("../assets/icons/file-text.png"),
            Self::ChevronDown => include_bytes!("../assets/icons/chevron-down.png"),
            Self::Lock => include_bytes!("../assets/icons/lock.png"),
            Self::Unplug => include_bytes!("../assets/icons/unplug.png"),
            Self::Search => include_bytes!("../assets/icons/search.png"),
            Self::PanelLeft => include_bytes!("../assets/icons/panel-left.png"),
            Self::PanelRight => include_bytes!("../assets/icons/panel-right.png"),
            Self::Fit => include_bytes!("../assets/icons/scan.png"),
            Self::Close => include_bytes!("../assets/icons/x.png"),
            Self::Layers => include_bytes!("../assets/icons/layers.png"),
            Self::More => include_bytes!("../assets/icons/ellipsis.png"),
            Self::Commit => include_bytes!("../assets/icons/git-commit-horizontal.png"),
            Self::Info => include_bytes!("../assets/icons/info.png"),
            Self::Copy => include_bytes!("../assets/icons/copy.png"),
            Self::Floating => include_bytes!("../assets/icons/app-window.png"),
            Self::Warning => include_bytes!("../assets/icons/triangle-alert.png"),
        }
    }
}

type Cache = BTreeMap<Icon, TextureHandle>;

fn texture(ctx: &egui::Context, icon: Icon) -> Option<TextureHandle> {
    let id = egui::Id::unique("talos-icon-textures");
    if let Some(handle) = ctx.data(|d| d.get_temp::<Cache>(id)?.get(&icon).cloned()) {
        return Some(handle);
    }
    let decoded = image::load_from_memory_with_format(icon.png(), image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let size = [decoded.width() as usize, decoded.height() as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
    let handle = ctx.load_texture(
        format!("talos-icon-{icon:?}"),
        image,
        TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(id)
            .insert(icon, handle.clone());
    });
    Some(handle)
}

/// Paints `icon` into `rect`, tinted.
pub fn paint(ui: &egui::Ui, icon: Icon, rect: egui::Rect, tint: Color32) {
    if let Some(texture) = texture(ui.ctx(), icon) {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            tint,
        );
    }
}

/// An inline, non-interactive icon of `size` points.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, tint: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    paint(ui, icon, rect, tint);
}

/// An icon-only button with an accessible name and a tooltip.
pub fn button(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let response = draw(ui, icon, label, selected);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, label));
    response
}

/// An icon-only option of a set (e.g. Docked / Floating): a button whose
/// `selected` state is in the accessibility tree, not only in its fill.
pub fn toggle(ui: &mut egui::Ui, icon: Icon, label: &str, on: bool) -> egui::Response {
    let response = draw(ui, icon, label, on);
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::accesskit::Role::Button, true, on, label));
    response
}

fn draw(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let tokens = crate::theme::Tokens::of(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 28.0), egui::Sense::click());
    if selected || response.hovered() {
        let fill = if selected {
            tokens.accent_weak
        } else {
            ui.visuals().widgets.hovered.weak_bg_fill
        };
        ui.painter().rect_filled(rect, tokens.radius, fill);
    }
    let tint = if selected || response.hovered() {
        tokens.text
    } else {
        tokens.muted
    };
    paint(
        ui,
        icon,
        egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)),
        tint,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            tokens.radius,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(label)
}
