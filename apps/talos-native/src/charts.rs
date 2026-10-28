//! Stored-result ECharts bridge. Rust remains the selection and scope authority.
use crate::{
    document::Results,
    state::{Action, ResultsQuery, Shown, State, Workspace},
};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
#[path = "charts_windows.rs"]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::PublicSmoke;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    project: String,
    branch: String,
    version: String,
    fingerprint: String,
    model_fingerprint: String,
    load_generation: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Scope {
    source: Source,
    generation: u64,
}
#[derive(Serialize)]
pub(crate) struct Packet {
    protocol: u8,
    scope: Scope,
    plot: lab_results::data::Plot,
    selected: Option<String>,
}
#[cfg(any(target_os = "windows", test))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    protocol: u8,
    #[serde(rename = "type")]
    kind: String,
    scope: Scope,
    // Required even when null; a missing field must not silently clear selection.
    story: serde_json::Value,
}
#[cfg(any(target_os = "windows", test))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Navigation {
    protocol: u8,
    #[serde(rename = "type")]
    kind: String,
    scope: Scope,
    workspace: String,
}
#[derive(Default)]
struct Bridge {
    epoch: u64,
    active: Option<(Scope, ResultsQuery, Vec<String>)>,
    rejected: u64,
}
fn source(state: &State) -> Option<Source> {
    let document = state.session.ready()?;
    let Results::Loaded(snapshot) = document.results() else {
        return None;
    };
    let version = document.version();
    Some(Source {
        project: state.session.project.name.clone(),
        branch: version.branch.clone(),
        version: version.version.clone(),
        fingerprint: snapshot.fingerprint.clone(),
        model_fingerprint: document
            .mesh()
            .revision()
            .source_sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        load_generation: state.session.ticket().generation,
    })
}
impl Bridge {
    fn suspend(&mut self) {
        self.active = None;
    }
    fn packet(&mut self, state: &State) -> Option<Packet> {
        if state.workspace != Workspace::Results {
            self.suspend();
            return None;
        }
        let next = source(state);
        let (Some(next), Shown::Plot(plot)) = (next, state.shown_profile()) else {
            self.suspend();
            return None;
        };
        let changed = self
            .active
            .as_ref()
            .is_none_or(|(scope, query, _)| scope.source != next || query != &state.results.query);
        if changed {
            self.epoch = self.epoch.checked_add(1)?;
            self.active = Some((
                Scope {
                    source: next,
                    generation: self.epoch,
                },
                state.results.query.clone(),
                plot.stories.clone(),
            ));
        }
        let (scope, _, _) = self.active.as_ref()?;
        Some(Packet {
            protocol: 1,
            scope: scope.clone(),
            plot,
            selected: state.results.story.clone(),
        })
    }
    #[cfg(any(target_os = "windows", test))]
    fn valid(&self, scope: &Scope, state: &State) -> bool {
        state.workspace == Workspace::Results
            && self.active.as_ref().is_some_and(|(active, query, _)| {
                scope == active
                    && query == &state.results.query
                    && source(state).as_ref() == Some(&active.source)
            })
    }
    #[cfg(any(target_os = "windows", test))]
    fn event(&mut self, raw: &str, state: &State) -> Result<Action, &'static str> {
        if raw.len() <= 1024
            && let Ok(message) = serde_json::from_str::<Navigation>(raw)
        {
            if message.protocol == 1
                && message.kind == "workspace"
                && self.valid(&message.scope, state)
                && let Some(workspace) = Workspace::ALL
                    .into_iter()
                    .find(|w| w.label() == message.workspace)
            {
                return Ok(Action::SelectWorkspace(workspace));
            }
            self.rejected += 1;
            return Err("stale or invalid chart navigation");
        }
        self.selection(raw, state)
    }
    #[cfg(any(target_os = "windows", test))]
    fn selection(&mut self, raw: &str, state: &State) -> Result<Action, &'static str> {
        let result = (|| {
            if raw.len() > 1024 {
                return Err("oversized chart message");
            }
            let message: Selection =
                serde_json::from_str(raw).map_err(|_| "invalid chart message")?;
            if message.protocol != 1
                || message.kind != "select"
                || !self.valid(&message.scope, state)
            {
                return Err("stale or foreign chart scope");
            }
            let story = match message.story {
                serde_json::Value::Null => None,
                serde_json::Value::String(name)
                    if self
                        .active
                        .as_ref()
                        .is_some_and(|(_, _, stories)| stories.contains(&name)) =>
                {
                    Some(name)
                }
                _ => return Err("unknown chart story"),
            };
            Ok(Action::SelectResultStory(story))
        })();
        if result.is_err() {
            self.rejected += 1;
        }
        result
    }
}

pub struct Charts {
    bridge: Bridge,
    error: Option<String>,
    headless: bool,
    #[cfg(target_os = "windows")]
    host: Option<windows::Host>,
}
impl Charts {
    pub fn headless() -> Self {
        Self {
            bridge: Bridge::default(),
            error: None,
            headless: true,
            #[cfg(target_os = "windows")]
            host: None,
        }
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(target_os = "windows")]
        {
            match windows::Host::new(cc) {
                Ok(host) => Self {
                    bridge: Bridge::default(),
                    error: None,
                    headless: false,
                    host: Some(host),
                },
                Err(error) => Self {
                    bridge: Bridge::default(),
                    error: Some(error),
                    headless: false,
                    host: None,
                },
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = cc;
            Self {
                bridge: Bridge::default(),
                error: Some("ECharts requires the Windows WebView2 host in this prototype.".into()),
                headless: false,
            }
        }
    }
    pub fn status(&self) -> &str {
        self.error.as_deref().unwrap_or(if self.headless {
            "ECharts host not attached (headless harness)"
        } else {
            "ECharts · WebView2"
        })
    }
    #[cfg(test)]
    pub(crate) fn refuse_for_test(&mut self) {
        self.error = Some("WebView2 creation refused (test control)".into());
        self.headless = false;
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn rejected(&self) -> u64 {
        self.bridge.rejected
    }
    pub fn actions(&mut self, state: &State) -> Vec<Action> {
        #[cfg(target_os = "windows")]
        {
            let mut actions = Vec::new();
            if let Some(host) = &mut self.host {
                for raw in host.messages() {
                    if host.accept_probe(&raw) {
                        continue;
                    }
                    if let Ok(action) = self.bridge.event(&raw, state) {
                        let leaving = matches!(action, Action::SelectWorkspace(_));
                        actions.push(action);
                        if leaving {
                            // Later queued events were validated for the old
                            // Results workspace; do not apply them after navigation.
                            break;
                        }
                    }
                }
            }
            actions
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = state;
            Vec::new()
        }
    }
    pub fn update(&mut self, state: &State, rect: Option<egui::Rect>, ctx: &egui::Context) {
        let packet = self.bridge.packet(state);
        #[cfg(target_os = "windows")]
        if let Some(host) = &mut self.host {
            let show = packet.is_some()
                && rect.is_some()
                && !egui::Popup::is_any_open(ctx)
                && !ctx.input(|i| i.viewport().minimized.unwrap_or(false));
            if let Err(error) = host.update(packet.as_ref(), rect, ctx.pixels_per_point(), show) {
                // Dropping the failed child also removes its HWND: never leave an
                // unhideable child over Model or replace it with another renderer.
                host.fail_smoke(ctx, &error);
                self.error = Some(error);
                self.host = None;
                self.bridge.suspend();
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (packet, rect, ctx);
    }
    #[cfg(target_os = "windows")]
    pub fn attach_smoke(&mut self, smoke: PublicSmoke) {
        if let Some(host) = &mut self.host {
            host.smoke = Some(smoke);
        }
    }
    #[cfg(target_os = "windows")]
    pub fn smoke_step(&mut self, state: &State, ctx: &egui::Context) -> Vec<Action> {
        self.host
            .as_mut()
            .map_or_else(Vec::new, |h| h.smoke_step(state, ctx, self.bridge.rejected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn ready() -> State {
        let project = crate::project::Project::public_results_fixture();
        let entry = project.versions[0].clone();
        let (mut state, _) = State::new(project);
        let ticket = state.session.ticket().clone();
        state
            .apply(Action::Loaded {
                ticket,
                result: crate::document::Document::load(&entry).map(Arc::new),
            })
            .expect("fixture");
        state
            .apply(Action::SelectWorkspace(Workspace::Results))
            .expect("Results");
        state
    }
    fn message(packet: &Packet) -> serde_json::Value {
        serde_json::json!({"protocol":1,"type":"select","scope":packet.scope,"story":"Roof"})
    }
    #[test]
    fn current_chart_selection_uses_the_existing_table_authority() {
        let mut state = ready();
        let mut bridge = Bridge::default();
        let packet = bridge.packet(&state).expect("stored profile");
        assert_eq!(packet.plot.series[0].values, vec![Some(0.002), Some(0.001)]);
        let action = bridge
            .selection(&message(&packet).to_string(), &state)
            .expect("current");
        state.apply(action).expect("table action");
        assert_eq!(state.results.story.as_deref(), Some("Roof"));
        assert_eq!(
            bridge.packet(&state).expect("selection").scope,
            packet.scope
        );
    }
    #[test]
    fn version_fingerprints_generations_and_unknown_fields_are_refused() {
        let state = ready();
        let mut bridge = Bridge::default();
        let packet = bridge.packet(&state).expect("packet");
        for field in [
            "branch",
            "version",
            "fingerprint",
            "model_fingerprint",
            "project",
        ] {
            let mut raw = message(&packet);
            raw["scope"]["source"][field] = serde_json::json!("foreign");
            assert!(
                bridge.selection(&raw.to_string(), &state).is_err(),
                "{field}"
            );
        }
        for field in ["generation", "load_generation"] {
            let mut raw = message(&packet);
            if field == "generation" {
                raw["scope"][field] = serde_json::json!(0)
            } else {
                raw["scope"]["source"][field] = serde_json::json!(0)
            };
            assert!(bridge.selection(&raw.to_string(), &state).is_err());
        }
        for raw in [
            "{}".to_owned(),
            "x".repeat(1025),
            {
                let mut v = message(&packet);
                v["extra"] = true.into();
                v.to_string()
            },
            {
                let mut v = message(&packet);
                v.as_object_mut().expect("object").remove("story");
                v.to_string()
            },
        ] {
            assert!(bridge.selection(&raw, &state).is_err());
        }
    }
    #[test]
    fn leaving_results_and_returning_never_revives_old_messages() {
        let mut state = ready();
        let mut bridge = Bridge::default();
        let old = bridge.packet(&state).expect("old");
        state
            .apply(Action::SelectWorkspace(Workspace::Model))
            .expect("Model");
        assert!(bridge.packet(&state).is_none());
        assert!(
            bridge
                .selection(&message(&old).to_string(), &state)
                .is_err()
        );
        state
            .apply(Action::SelectWorkspace(Workspace::Results))
            .expect("Results");
        let new = bridge.packet(&state).expect("new");
        assert_ne!(new.scope, old.scope);
        assert!(
            bridge
                .selection(&message(&old).to_string(), &state)
                .is_err()
        );
        assert!(bridge.selection(&message(&new).to_string(), &state).is_ok());
    }
    #[test]
    fn changing_query_refuses_queued_messages_before_the_next_frame() {
        let mut state = ready();
        let mut bridge = Bridge::default();
        let old = bridge.packet(&state).expect("old");
        state
            .apply(Action::SetDirection(crate::state::Direction::Y))
            .expect("direction");
        assert!(
            bridge
                .selection(&message(&old).to_string(), &state)
                .is_err()
        );
        let new = bridge.packet(&state).expect("new");
        assert_ne!(old.scope, new.scope);
    }

    #[test]
    fn chart_keyboard_navigation_is_scoped_and_expires_on_model() {
        let mut state = ready();
        let mut bridge = Bridge::default();
        let packet = bridge.packet(&state).expect("packet");
        let raw=serde_json::json!({"protocol":1,"type":"workspace","scope":packet.scope,"workspace":"Model"}).to_string();
        let action = bridge.event(&raw, &state).expect("native shortcut");
        state.apply(action).expect("Model");
        assert_eq!(state.workspace, Workspace::Model);
        assert!(bridge.event(&raw, &state).is_err());
    }
    #[test]
    fn offline_page_uses_the_bundled_runtime_without_network_or_html_tooltips() {
        let html = include_str!("chart.html");
        assert!(html.contains("connect-src 'none'"));
        assert!(html.contains("renderMode:'richText'"));
        assert!(html.contains("/*__ECHARTS_RUNTIME__*/"));
        assert!(!html.contains("https://"));
        assert!(html.contains("connectNulls:false"));
    }
}
