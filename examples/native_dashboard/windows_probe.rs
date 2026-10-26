//! Opt-in egui overlay/text field for owner-run Windows airspace and IME tests.
//! Nothing typed here is persisted, logged, or included in smoke reports.

#[derive(Default)]
pub(crate) struct WindowsProbe {
    open: bool,
    text: String,
    text_focused: bool,
    request_focus: bool,
    placement: Option<Placement>,
}

#[derive(Clone, Copy)]
enum Placement {
    Native,
    OverChart,
}

impl WindowsProbe {
    pub(crate) fn controls(&mut self, ui: &mut egui::Ui) {
        if ui.checkbox(&mut self.open, "Windows input probe").changed() && self.open {
            self.placement = Some(Placement::Native);
        }
        if self.open {
            if ui.button("Probe in native area").clicked() {
                self.placement = Some(Placement::Native);
            }
            if ui.button("Probe over charts").clicked() {
                self.placement = Some(Placement::OverChart);
            }
            if ui.button("Focus probe text").clicked() {
                self.request_focus = true;
            }
        }
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context, chart_rect: Option<egui::Rect>) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        let mut window = egui::Window::new("Windows composition / IME probe")
            .id(egui::Id::unique("dashboard-windows-input-probe"))
            .open(&mut open)
            .collapsible(false)
            .default_size([350.0, 220.0]);
        if let Some(placement) = self.placement.take() {
            let pos = match (placement, chart_rect) {
                (Placement::OverChart, Some(rect)) => rect.min + egui::vec2(16.0, 16.0),
                _ => egui::pos2(12.0, 180.0),
            };
            window = window.current_pos(pos);
        }
        window.show(ctx, |ui| self.content(ui));
        self.open = open;
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        ui.label("egui overlay in the parent HWND; charts remain visible.");
        ui.label(
            "Observe occlusion/input failures; this probe does not repair child-HWND z-order.",
        );
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.text)
                .id_salt("dashboard-probe-text")
                .char_limit(128)
                .hint_text("Type synthetic Unicode / IME text here"),
        );
        if std::mem::take(&mut self.request_focus) {
            response.request_focus();
        }
        self.text_focused = response.has_focus();
        ui.label(if self.text_focused {
            "egui field focus: yes (not an OS IME certification)"
        } else {
            "egui field focus: no"
        });
        ui.horizontal(|ui| {
            if ui.button("Clear probe text").clicked() {
                self.text.clear();
            }
            ui.menu_button("Probe popup", |ui| {
                ui.label("Synthetic popup: inspect clipping and hit targets.");
                if ui.button("Close popup").clicked() {
                    ui.close();
                }
            });
        });
        ui.small("Use the top-bar controls if this overlay becomes obscured by WebView2. No text is saved or sent to charts.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(probe: &mut WindowsProbe, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| probe.content(ui));
        // This input-routing unit test has no painter or GPU texture consumer.
        output.textures_delta.clear();
    }

    #[test]
    fn probe_text_routes_headless_unicode_only_while_its_field_has_focus() {
        let ctx = egui::Context::default();
        let mut probe = WindowsProbe::default();
        frame(&mut probe, &ctx, vec![egui::Event::Text("ignored".into())]);
        assert!(probe.text.is_empty());
        probe.request_focus = true;
        frame(&mut probe, &ctx, vec![]);
        frame(
            &mut probe,
            &ctx,
            vec![egui::Event::Text("Synthetic 日本語 Tiếng Việt".into())],
        );
        assert_eq!(probe.text, "Synthetic 日本語 Tiếng Việt");
        assert!(probe.text_focused);
    }
}
