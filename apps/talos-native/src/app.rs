//! The application host: runs effects, routes keyboard shortcuts, draws the
//! chrome (header, workspace tabs, status line) and the active workspace page.
use crate::chart_bridge::EmbeddedChart;
use crate::loader::{LoadMode, Loader};
use crate::project::Project;
use crate::state::{Action, DocumentState, Refusal, State, Workspace};
use crate::view::ModelView;

/// Below this window width the header collapses and Model shows one
/// contextual panel at a time.
pub const COMPACT_WIDTH: f32 = 1100.0;

pub struct App {
    state: State,
    view: Box<dyn ModelView>,
    loader: Loader,
    last_refusal: Option<Refusal>,
    /// The Results chart outside egui (`ECharts` in a `WebView2` child on
    /// Windows). An unavailable host is explicit, without a renderer fallback.
    chart: Option<Box<dyn EmbeddedChart>>,
    chart_start_error: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, mode: LoadMode, project: Project) -> Self {
        crate::theme::install(&cc.egui_ctx);
        Self::with_parts(
            project,
            Box::new(crate::native_view::NativeModelView::new(
                cc.wgpu_render_state.as_ref(),
            )),
            Loader::new(mode, Some(cc.egui_ctx.clone())),
        )
        .with_platform_chart(cc)
    }
    /// Windows: the Results chart is `ECharts` in a `WebView2` child window.
    /// A creation failure stays visible in the shell; no renderer replaces it.
    #[cfg(target_os = "windows")]
    fn with_platform_chart(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        match crate::webview_chart::WebViewChart::new(cc) {
            Ok(chart) => self.with_chart(Box::new(chart)),
            Err(error) => {
                self.chart_start_error = Some(error);
                self
            }
        }
    }
    /// The current prototype requires the Windows host for ECharts.
    #[cfg(not(target_os = "windows"))]
    fn with_platform_chart(mut self, _cc: &eframe::CreationContext<'_>) -> Self {
        self.chart_start_error = Some("ECharts requires Windows WebView2 in this prototype".into());
        self
    }
    pub fn with_parts(project: Project, view: Box<dyn ModelView>, loader: Loader) -> Self {
        let (state, effects) = State::new(project);
        let mut app = Self {
            state,
            view,
            loader,
            last_refusal: None,
            chart: None,
            chart_start_error: None,
        };
        for effect in effects {
            app.loader.run(effect);
        }
        app
    }
    /// Draws the Results chart with `chart` whenever it is available.
    #[must_use]
    pub fn with_chart(mut self, chart: Box<dyn EmbeddedChart>) -> Self {
        self.chart = Some(chart);
        self
    }
    pub fn chart_error(&self) -> Option<&str> {
        self.chart_start_error
            .as_deref()
            .or_else(|| self.chart.as_ref().and_then(|chart| chart.error()))
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn view(&self) -> &dyn ModelView {
        self.view.as_ref()
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

    fn shortcuts(&mut self, ctx: &egui::Context) -> Vec<Action> {
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
                if input.consume_key(Modifiers::NONE, Key::F) {
                    self.view.fit();
                }
            });
        }
        actions
    }

    /// One 48 pt row: identity, version, workspaces and the read-only status.
    /// Below `COMPACT_WIDTH` the project name and status text collapse and the
    /// later workspaces move into More.
    fn header(&self, ui: &mut egui::Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        let session = &self.state.session;
        let tokens = crate::theme::Tokens::of(ui);
        let compact = ui.available_width() < COMPACT_WIDTH;
        let row = egui::vec2(ui.available_width(), 48.0);
        ui.allocate_ui_with_layout(
            row,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                brand(ui);
                ui.add_space(10.0);
                if !compact {
                    ui.label(
                        egui::RichText::new(&session.project.name)
                            .family(crate::theme::medium())
                            .color(tokens.text),
                    );
                    ui.label(egui::RichText::new("/").color(tokens.faint));
                }
                let requested = session.requested();
                ui.label(egui::RichText::new(&requested.branch).color(tokens.muted));
                ui.label(egui::RichText::new("/").color(tokens.faint));
                let version = egui::ComboBox::from_id_salt("talos-version")
                    .selected_text(
                        egui::RichText::new(&requested.version).family(crate::theme::semibold()),
                    )
                    .width(56.0)
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
                crate::theme::pill(
                    ui,
                    authority.label(),
                    tokens.synthetic,
                    tokens.synthetic_weak,
                    authority.detail(),
                );
                if let DocumentState::Loading(ticket) = &session.document {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new(format!("Loading {}…", ticket.version))
                            .size(crate::theme::SMALL)
                            .color(tokens.muted),
                    );
                }
                ui.add_space(14.0);
                if let Some(workspace) = crate::tabs::show(ui, self.state.workspace, compact) {
                    actions.push(Action::SelectWorkspace(workspace));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    status(ui, compact);
                });
            },
        );
        actions
    }

    /// Only when something is wrong; a healthy session shows no banner.
    fn warning(&self, ui: &mut egui::Ui) {
        if let DocumentState::Failed { ticket, message } = &self.state.session.document {
            let tokens = crate::theme::Tokens::of(ui);
            egui::Frame::new()
                .fill(tokens.warning_fill)
                .inner_margin(egui::Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        crate::icons::show(
                            ui,
                            crate::icons::Icon::Warning,
                            16.0,
                            tokens.warning_text,
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "Could not load {}: {message}",
                                ticket.version
                            ))
                            .color(tokens.warning_text),
                        );
                    });
                });
        }
    }
}

/// The TALOS mark: a rounded accent tile with a column, and the wordmark.
fn brand(ui: &mut egui::Ui) {
    let tokens = crate::theme::Tokens::of(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 5.0, tokens.accent);
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            rect.left_top() + egui::vec2(8.0, 5.0),
            rect.right_bottom() - egui::vec2(8.0, 4.0),
        ),
        1.0,
        tokens.chrome,
    );
    ui.label(
        egui::RichText::new("TALOS")
            .family(crate::theme::semibold())
            .extra_letter_spacing(1.2)
            .color(tokens.text),
    );
}

/// Read-only and ETABS status; icon-only (with tooltips) when compact.
fn status(ui: &mut egui::Ui, compact: bool) {
    use crate::icons::{Icon, show};
    let tokens = crate::theme::Tokens::of(ui);
    // Inside the header's right-to-left layout the first item drawn is the
    // rightmost, so each item adds its text before its icon.
    let item = |ui: &mut egui::Ui, icon: Icon, text: &str, hover: &str| {
        let response = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                if !compact {
                    ui.label(
                        egui::RichText::new(text)
                            .size(crate::theme::SMALL)
                            .color(tokens.muted),
                    );
                }
                show(ui, icon, 14.0, tokens.muted);
            })
            .response
            .on_hover_text(hover);
        if compact {
            // The icon alone still names its state for assistive technology.
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, text)
            });
        }
    };
    // Right-to-left layout: the last item drawn is leftmost.
    item(
        ui,
        Icon::Unplug,
        "ETABS not connected",
        "ETABS not connected: this prototype never starts or controls ETABS.",
    );
    item(
        ui,
        Icon::Lock,
        "Read-only",
        "Read-only: nothing in this prototype writes files, commits or runs analysis.",
    );
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
        egui::Panel::top("talos-header")
            .frame(
                egui::Frame::new()
                    .fill(tokens.chrome)
                    .stroke(egui::Stroke::new(1.0, tokens.chrome_line))
                    .inner_margin(egui::Margin::symmetric(14, 0)),
            )
            .show(ui, |ui| actions.extend(self.header(ui)));
        // Only drawn when something is wrong; an empty panel would still take space.
        if matches!(self.state.session.document, DocumentState::Failed { .. })
            || self.chart_error().is_some()
        {
            egui::Panel::top("talos-warning")
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    self.warning(ui);
                    if let Some(error) = self.chart_error() {
                        ui.colored_label(
                            tokens.warning_text,
                            format!("ECharts / WebView2 unavailable: {error}"),
                        );
                    }
                });
        }
        let workspace = self.state.workspace;
        let rendered_ticket = self.state.session.ticket().clone();
        let rendered_query = self.state.results.query.clone();
        if workspace != Workspace::Results
            && let Some(chart) = &mut self.chart
        {
            // Hide before Model draws, not one input/frame later.
            let _discarded = chart.sync(&ctx, None, None);
        }
        let embedded = self.chart.as_ref().is_some_and(|chart| chart.available());
        let mut embed = None;
        egui::CentralPanel::no_frame().show(ui, |ui| match workspace {
            Workspace::Model => {
                actions.extend(crate::pages::model::show(
                    ui,
                    &self.state,
                    self.view.as_mut(),
                ));
            }
            Workspace::Results => {
                let (page, chart) = crate::pages::results::show(ui, &self.state, embedded);
                actions.extend(page);
                embed = chart;
            }
            other => crate::pages::placeholder::show(ui, other),
        });
        // Apply the native controls first: queued chart messages must not be
        // accepted against the previous query/version/workspace in this frame.
        if !actions.is_empty() {
            // A native table click changes the chart selection after layout.
            ctx.request_repaint();
        }
        for action in actions {
            self.dispatch(action);
        }
        if self.state.workspace != workspace
            || self.state.session.ticket() != &rendered_ticket
            || self.state.results.query != rendered_query
        {
            embed = None;
            ctx.request_repaint();
        }
        let mut chart_actions = Vec::new();
        if let Some(chart) = &mut self.chart {
            // A child window draws above everything egui paints, so it hides
            // while an egui popup (a selector's list) is open, and while the
            // window is minimized. Its content stays, so it reappears as is.
            let covered = egui::Popup::is_any_open(&ctx)
                || ctx.input(|input| input.viewport().minimized == Some(true));
            let (area, content) = match embed {
                Some(embed) => ((!covered).then_some(embed.area), Some(embed.content)),
                None => (None, None),
            };
            let requested = chart.sync(&ctx, area, content);
            if !requested.is_empty() {
                // The chart shows the result (e.g. the selected band) only
                // when it is next synced, so draw another frame.
                ctx.request_repaint();
            }
            chart_actions.extend(requested);
        }
        for action in chart_actions {
            self.dispatch(action);
        }
        if self.state.workspace != Workspace::Results
            && let Some(chart) = &mut self.chart
        {
            // Chart shortcuts are dispatched after sync; hide immediately too.
            let _discarded = chart.sync(&ctx, None, None);
        }
    }
}
