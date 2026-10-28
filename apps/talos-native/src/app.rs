//! The application host: runs effects, routes keyboard shortcuts, draws the
//! chrome (header, workspace tabs, status line) and the active workspace page.
use crate::loader::{LoadMode, Loader};
use crate::project::Project;
use crate::state::{Action, DocumentState, Refusal, State, Workspace};
use crate::view::{ModelView, PlaceholderView};

pub struct App {
    state: State,
    view: Box<dyn ModelView>,
    loader: Loader,
    last_refusal: Option<Refusal>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, mode: LoadMode) -> Self {
        crate::theme::install(&cc.egui_ctx);
        Self::with_parts(
            Project::public_demo(),
            Box::new(PlaceholderView),
            Loader::new(mode, Some(cc.egui_ctx.clone())),
        )
    }
    pub fn with_parts(project: Project, view: Box<dyn ModelView>, loader: Loader) -> Self {
        let (state, effects) = State::new(project);
        let mut app = Self {
            state,
            view,
            loader,
            last_refusal: None,
        };
        for effect in effects {
            app.loader.run(effect);
        }
        app
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn loader_mut(&mut self) -> &mut Loader {
        &mut self.loader
    }
    /// The most recent refused action, for diagnostics and tests.
    pub fn last_refusal(&self) -> Option<Refusal> {
        self.last_refusal
    }
    pub fn dispatch(&mut self, action: Action) {
        match self.state.apply(action) {
            Ok(effects) => {
                for effect in effects {
                    self.loader.run(effect);
                }
            }
            Err(refusal) => {
                log::debug!("refused: {refusal:?}");
                self.last_refusal = Some(refusal);
            }
        }
    }

    fn shortcuts(&self, ctx: &egui::Context) -> Vec<Action> {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let mut actions = Vec::new();
        let digits = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5];
        ctx.input_mut(|input| {
            for (key, workspace) in digits.into_iter().zip(Workspace::ALL) {
                if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, key)) {
                    actions.push(Action::SelectWorkspace(workspace));
                }
            }
            if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::PageDown)) {
                actions.push(Action::NextWorkspace);
            }
            if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::PageUp)) {
                actions.push(Action::PreviousWorkspace);
            }
        });
        if self.state.workspace == Workspace::Model && !ctx.egui_wants_keyboard_input() {
            ctx.input_mut(|input| {
                if input.consume_key(Modifiers::NONE, Key::E) {
                    actions.push(Action::ToggleExplorer);
                }
                if input.consume_key(Modifiers::NONE, Key::I) {
                    actions.push(Action::ToggleInspector);
                }
            });
        }
        actions
    }

    fn header(&self, ui: &mut egui::Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        let session = &self.state.session;
        let tokens = crate::theme::Tokens::of(ui);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("TALOS").strong());
            ui.add_space(10.0);
            ui.label(&session.project.name);
            ui.label(egui::RichText::new("›").color(tokens.muted));
            let requested = session.requested();
            ui.label(&requested.branch);
            ui.label(egui::RichText::new("›").color(tokens.muted));
            let version = egui::ComboBox::from_id_salt("talos-version")
                .selected_text(&requested.version)
                .show_ui(ui, |ui| {
                    for entry in &session.project.versions {
                        let text = format!("{}   {}", entry.id.version, entry.summary);
                        if ui.selectable_label(&entry.id == requested, text).clicked() {
                            actions.push(Action::SwitchVersion(entry.id.clone()));
                        }
                    }
                })
                .response;
            ui.ctx()
                .accesskit_node_builder(version.id, |node| node.set_label("Version"));
            version.on_hover_text("Version");
            let authority = session.project.authority;
            crate::theme::tag(ui, authority.label(), authority.detail());
            if let DocumentState::Loading(ticket) = &session.document {
                ui.spinner();
                ui.label(
                    egui::RichText::new(format!("Loading {}…", ticket.version))
                        .small()
                        .color(tokens.muted),
                );
            }
        });
        actions
    }

    /// Only when something is wrong; a healthy session shows no banner.
    fn warning(&self, ui: &mut egui::Ui) {
        if let DocumentState::Failed { ticket, message } = &self.state.session.document {
            let tokens = crate::theme::Tokens::of(ui);
            egui::Frame::new()
                .fill(tokens.warning_fill)
                .inner_margin(egui::Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(format!(
                            "Could not load {}: {message}",
                            ticket.version
                        ))
                        .color(tokens.warning_text),
                    );
                });
        }
    }

    fn status(ui: &mut egui::Ui) {
        let tokens = crate::theme::Tokens::of(ui);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Read-only").small().color(tokens.muted))
                .on_hover_text("Nothing in this prototype writes files, commits or runs analysis.");
            ui.label(egui::RichText::new("·").small().color(tokens.muted));
            ui.label(
                egui::RichText::new("ETABS not connected")
                    .small()
                    .color(tokens.muted),
            )
            .on_hover_text("This prototype never starts or controls ETABS.");
        });
    }
}

impl eframe::App for App {
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        for (ticket, result) in self.loader.drain() {
            self.dispatch(Action::Loaded { ticket, result });
        }
        let ctx = ui.ctx().clone();
        let mut actions = self.shortcuts(&ctx);
        let tokens = crate::theme::Tokens::of(ui);
        let chrome = egui::Frame::new()
            .fill(tokens.chrome)
            .inner_margin(egui::Margin::symmetric(12, 6));
        egui::Panel::top("talos-header")
            .frame(chrome)
            .show(ui, |ui| actions.extend(self.header(ui)));
        // Only drawn when something is wrong; an empty panel would still take space.
        if matches!(self.state.session.document, DocumentState::Failed { .. }) {
            egui::Panel::top("talos-warning")
                .frame(egui::Frame::NONE)
                .show(ui, |ui| self.warning(ui));
        }
        egui::Panel::top("talos-tabs")
            .frame(chrome.inner_margin(egui::Margin::symmetric(8, 0)))
            .show(ui, |ui| {
                if let Some(workspace) = crate::tabs::show(ui, self.state.workspace) {
                    actions.push(Action::SelectWorkspace(workspace));
                }
            });
        egui::Panel::bottom("talos-status")
            .frame(chrome.inner_margin(egui::Margin::symmetric(12, 3)))
            .show(ui, Self::status);
        let workspace = self.state.workspace;
        egui::CentralPanel::no_frame().show(ui, |ui| match workspace {
            Workspace::Model => {
                actions.extend(crate::pages::model::show(
                    ui,
                    &self.state,
                    self.view.as_mut(),
                ));
            }
            Workspace::Results => actions.extend(crate::pages::results::show(ui, &self.state)),
            other => crate::pages::placeholder::show(ui, other),
        });
        for action in actions {
            self.dispatch(action);
        }
    }
}
