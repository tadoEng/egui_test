//! State-transition tests: no egui, no window, no threads.
use crate::document::{self, Document};
use crate::project::{Authority, Project, Source, VersionEntry, VersionId};
use crate::state::{
    Action, DocumentState, Effect, Metric, Refusal, State, StoryRange, Ticket, Workspace,
};
use lab_e2k_mesh::ShellPlacement;
use std::sync::Arc;

fn v(version: &str) -> VersionId {
    VersionId::new("main", version)
}
/// Runs a load effect synchronously, returning the response it would send.
fn load(effect: &Effect) -> (Ticket, Result<Arc<Document>, String>) {
    let Effect::Load { ticket, entry } = effect;
    (ticket.clone(), Document::load(entry).map(Arc::new))
}
fn deliver(state: &mut State, effect: &Effect) -> Result<Vec<Effect>, Refusal> {
    let (ticket, result) = load(effect);
    state.apply(Action::Loaded { ticket, result })
}
fn ready_demo() -> State {
    let (mut state, effects) = State::new(Project::public_demo());
    assert_eq!(effects.len(), 1, "one initial load");
    deliver(&mut state, &effects[0]).expect("initial load is current");
    state
}
fn switch(state: &mut State, version: &str) -> Effect {
    let mut effects = state
        .apply(Action::SwitchVersion(v(version)))
        .expect("known version");
    assert_eq!(effects.len(), 1, "a switch issues exactly one load");
    effects.remove(0)
}
fn shell(document: &Document, name: &str) -> lab_e2k_mesh::ShellSource {
    document
        .mesh()
        .entities()
        .iter()
        .find(|e| e.shell.is_some() && e.key.to_string().contains(name))
        .and_then(|e| e.shell.clone())
        .expect("named shell")
}
fn leaked(text: String) -> &'static [u8] {
    Box::leak(text.into_bytes().into_boxed_slice())
}

#[test]
fn starts_on_model_loading_the_latest_version() {
    let (state, effects) = State::new(Project::public_demo());
    assert_eq!(state.workspace, Workspace::Model);
    assert_eq!(state.session.requested(), &v("v2"));
    assert!(matches!(state.session.document, DocumentState::Loading(_)));
    assert!(state.session.scope().is_none(), "no scope before a load");
    let [Effect::Load { ticket, .. }] = effects.as_slice() else {
        panic!("expected one load, got {effects:?}");
    };
    assert_eq!(ticket.version, v("v2"));
}

#[test]
fn the_two_demo_versions_differ_only_in_a_source_backed_slab_insertion() {
    let project = Project::public_demo();
    assert_eq!(project.authority, Authority::PublicSyntheticDemo);
    let [first, second] = project.versions.as_slice() else {
        panic!("two versions");
    };
    let (a, b) = (
        Document::load(first).expect("v1"),
        Document::load(second).expect("v2"),
    );
    assert_ne!(
        a.id().revision.source_sha256,
        b.id().revision.source_sha256,
        "distinct source bytes"
    );
    let (sa, sb) = (shell(&a, "S"), shell(&b, "S"));
    assert_eq!(sa.insertion_point.as_deref(), Some("MIDDLE"));
    assert_eq!(sb.insertion_point.as_deref(), Some("TOP"));
    assert_eq!(sa.placement, ShellPlacement::CentredMiddle);
    assert_eq!(sb.placement, ShellPlacement::FlatAtDeclaredFace);
    assert_eq!(
        shell(&a, "W").placement,
        shell(&b, "W").placement,
        "the wall is unchanged"
    );
}

#[test]
fn source_lengths_are_converted_explicitly_and_shown_with_their_source_value() {
    let document = Document::load(&Project::public_demo().versions[0]).expect("v1");
    let units = document.mesh().source_units();
    assert_eq!(document::unit_names(units), ["kip", "ft", "°F"]);
    let thickness = shell(&document, "S").thickness_m.expect("declared");
    assert!((thickness - 0.5 * 0.3048).abs() < 1e-12, "0.5 ft in metres");
    assert_eq!(document::length(thickness, units), "0.1524 m (0.5 ft)");
}

#[test]
fn only_the_current_ticket_is_applied_after_rapid_switches_and_late_responses() {
    let (mut state, mut effects) = State::new(Project::public_demo()); // g1: v2
    effects.push(switch(&mut state, "v1")); // g2
    effects.push(switch(&mut state, "v2")); // g3
    effects.push(switch(&mut state, "v1")); // g4
    let generations: Vec<_> = effects
        .iter()
        .map(|Effect::Load { ticket, .. }| ticket.generation)
        .collect();
    assert_eq!(generations, [1, 2, 3, 4]);
    // Responses arrive out of order; only g4 (v1) is current.
    for at in [2, 0] {
        assert_eq!(
            deliver(&mut state, &effects[at]),
            Err(Refusal::StaleResponse)
        );
        assert!(matches!(state.session.document, DocumentState::Loading(_)));
    }
    deliver(&mut state, &effects[3]).expect("current");
    assert_eq!(
        deliver(&mut state, &effects[1]),
        Err(Refusal::StaleResponse),
        "an older v1 response does not replace the current v1 document"
    );
    let DocumentState::Ready { ticket, document } = &state.session.document else {
        panic!("ready");
    };
    assert_eq!((ticket.generation, document.version()), (4, &v("v1")));
    assert_eq!(state.stale_discarded, 3);
}

#[test]
fn a_late_response_never_shows_the_previous_version_while_the_next_loads() {
    let (mut state, effects) = State::new(Project::public_demo());
    deliver(&mut state, &effects[0]).expect("v2");
    let _pending = switch(&mut state, "v1");
    assert!(state.session.ready().is_none(), "v2 is gone while v1 loads");
    assert_eq!(
        deliver(&mut state, &effects[0]),
        Err(Refusal::StaleResponse)
    );
    assert!(state.session.ready().is_none());
    assert_eq!(state.session.requested(), &v("v1"));
}

#[test]
fn a_response_for_the_wrong_version_fails_closed() {
    let (mut state, effects) = State::new(Project::public_demo());
    let Effect::Load { ticket, .. } = &effects[0];
    let wrong = Document::load(&Project::public_demo().versions[0]).expect("v1");
    assert_eq!(
        state.apply(Action::Loaded {
            ticket: ticket.clone(),
            result: Ok(Arc::new(wrong)),
        }),
        Err(Refusal::MismatchedResponse)
    );
    assert!(matches!(
        state.session.document,
        DocumentState::Failed { .. }
    ));
    assert!(state.session.ready().is_none());
}

#[test]
fn a_failed_load_can_be_retried_and_a_ready_version_is_not_reloaded() {
    let (mut state, effects) = State::new(Project::public_demo());
    let Effect::Load { ticket, .. } = &effects[0];
    state
        .apply(Action::Loaded {
            ticket: ticket.clone(),
            result: Err("refused".to_owned()),
        })
        .expect("current");
    assert!(matches!(
        state.session.document,
        DocumentState::Failed { .. }
    ));
    let retry = switch(&mut state, "v2");
    deliver(&mut state, &retry).expect("retry");
    assert!(state.session.ready().is_some());
    assert_eq!(state.apply(Action::SwitchVersion(v("v2"))), Ok(Vec::new()));
    assert_eq!(
        state.apply(Action::SwitchVersion(v("v9"))),
        Err(Refusal::UnknownVersion)
    );
}

#[test]
fn selection_is_scoped_to_version_generation_and_geometry() {
    let mut state = ready_demo();
    let scope = state.session.scope().expect("ready");
    state
        .apply(Action::Pick {
            scope,
            entity: Some(0),
            toggle: false,
        })
        .expect("in scope");
    assert_eq!(state.model.selected(scope), [0]);
    assert_eq!(
        state.apply(Action::Pick {
            scope,
            entity: Some(999),
            toggle: false,
        }),
        Err(Refusal::UnknownEntity)
    );
    let effect = switch(&mut state, "v1");
    assert_eq!(
        state.apply(Action::Pick {
            scope,
            entity: Some(1),
            toggle: false,
        }),
        Err(Refusal::NoDocument),
        "nothing is pickable while loading"
    );
    deliver(&mut state, &effect).expect("v1");
    let next = state.session.scope().expect("ready");
    assert_ne!(scope, next);
    assert!(state.model.selected(next).is_empty(), "switch cleared it");
    assert_eq!(state.model.selected(scope), [] as [usize; 0]);
    assert_eq!(
        state.apply(Action::Pick {
            scope,
            entity: Some(1),
            toggle: false,
        }),
        Err(Refusal::StaleSelection),
        "a pick made against v2 is refused on v1"
    );
}

#[test]
fn identical_bytes_as_another_version_still_expire_the_selection() {
    let source = Project::public_demo().versions[0].source;
    let project = Project {
        name: "Same bytes".to_owned(),
        authority: Authority::PublicSyntheticDemo,
        versions: ["a", "b"]
            .map(|name| VersionEntry {
                id: v(name),
                summary: "same bytes",
                source,
            })
            .to_vec(),
    };
    let (mut state, effects) = State::new(project);
    deliver(&mut state, &effects[0]).expect("b");
    let old = state.session.scope().expect("ready");
    state
        .apply(Action::Pick {
            scope: old,
            entity: Some(0),
            toggle: false,
        })
        .expect("in scope");
    let effect = switch(&mut state, "a");
    deliver(&mut state, &effect).expect("a");
    let new = state.session.scope().expect("ready");
    assert_eq!(old.0, new.0, "same geometry revision");
    assert_ne!(old.1, new.1, "different load generation");
    assert_eq!(
        state.apply(Action::Pick {
            scope: old,
            entity: Some(0),
            toggle: true,
        }),
        Err(Refusal::StaleSelection)
    );
}

#[test]
fn workspaces_keep_their_own_state_across_tab_switches() {
    let mut state = ready_demo();
    let scope = state.session.scope().expect("ready");
    for action in [
        Action::ToggleExplorer,
        Action::Pick {
            scope,
            entity: Some(1),
            toggle: false,
        },
        Action::SelectWorkspace(Workspace::Results),
        Action::SetMetric(Metric::StoryShear),
        Action::NextWorkspace,
        Action::SelectWorkspace(Workspace::Model),
    ] {
        state.apply(action).expect("accepted");
    }
    assert!(!state.model.explorer_open);
    assert!(state.model.inspector_open);
    assert_eq!(state.model.selected(scope), [1]);
    assert_eq!(state.results.query.metric, Metric::StoryShear);
    state.apply(Action::PreviousWorkspace).expect("wraps");
    assert_eq!(state.workspace, Workspace::Reports);
    state.apply(Action::NextWorkspace).expect("wraps");
    assert_eq!(state.workspace, Workspace::Model);
}

#[test]
fn only_model_and_results_are_available_and_the_rest_say_why() {
    for workspace in Workspace::ALL {
        let reason = workspace.unavailable_reason();
        match workspace {
            Workspace::Model | Workspace::Results => assert!(reason.is_none()),
            _ => assert!(reason.is_some_and(|r| r.len() > 40), "{workspace:?}"),
        }
    }
}

#[test]
fn the_results_query_is_validated_against_the_version_and_never_invents_a_case() {
    let mut state = ready_demo();
    assert_eq!(state.results.query.version, Some(v("v2")));
    assert_eq!(state.results.query.case, None);
    let range = |top: &str, bottom: &str| {
        Some(StoryRange {
            top: top.to_owned(),
            bottom: bottom.to_owned(),
        })
    };
    state
        .apply(Action::SetStoryRange(range("Roof", "Roof")))
        .expect("valid");
    assert_eq!(
        state.apply(Action::SetStoryRange(range("Base", "Roof"))),
        Err(Refusal::UnknownStory),
        "inverted"
    );
    assert_eq!(
        state.apply(Action::SetStoryRange(range("Roof", "Level 9"))),
        Err(Refusal::UnknownStory)
    );
    assert_eq!(state.results.query.stories, range("Roof", "Roof"));
    assert_eq!(
        crate::pages::results::describe(&state.results.query),
        "Story drift · X · no case · Roof–Roof · main/v2"
    );
}

#[test]
fn a_story_range_missing_from_the_next_version_is_reset_with_a_notice() {
    let Source::Embedded(bytes) = Project::public_demo().versions[0].source;
    let renamed = String::from_utf8(bytes.to_vec())
        .expect("utf-8")
        .replace("\"Roof\"", "\"Level 2\"");
    let mut project = Project::public_demo();
    project.versions[0].source = Source::Embedded(leaked(renamed));
    let (mut state, effects) = State::new(project);
    deliver(&mut state, &effects[0]).expect("v2");
    state
        .apply(Action::SetStoryRange(Some(StoryRange {
            top: "Roof".to_owned(),
            bottom: "Base".to_owned(),
        })))
        .expect("valid in v2");
    let effect = switch(&mut state, "v1");
    deliver(&mut state, &effect).expect("v1");
    assert_eq!(state.results.query.stories, None);
    assert_eq!(state.results.query.version, Some(v("v1")));
    assert!(
        state
            .results
            .notice
            .as_deref()
            .is_some_and(|n| n.contains("Roof–Base") && n.contains("main/v1")),
        "{:?}",
        state.results.notice
    );
    // The notice describes the v1 load only; it must not follow the user back.
    let effect = switch(&mut state, "v2");
    deliver(&mut state, &effect).expect("v2");
    assert_eq!(state.results.notice, None, "stale notice about main/v1");
    assert_eq!(state.results.query.version, Some(v("v2")));
}
