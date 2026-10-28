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
fn the_two_demo_versions_differ_in_source_backed_slab_insertion_and_beam_depth() {
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
    let (sa, sb) = (shell(&a, "|F|L12"), shell(&b, "|F|L12"));
    assert_eq!(sa.insertion_point.as_deref(), Some("MIDDLE"));
    assert_eq!(sb.insertion_point.as_deref(), Some("TOP"));
    assert_eq!(sa.placement, ShellPlacement::CentredMiddle);
    assert_eq!(sb.placement, ShellPlacement::FlatAtDeclaredFace);
    assert_eq!(
        shell(&a, "|W1|L12").placement,
        shell(&b, "|W1|L12").placement,
        "the wall is unchanged"
    );
}

#[test]
fn source_lengths_are_converted_explicitly_and_shown_with_their_source_value() {
    let document = Document::load(&Project::public_demo().versions[0]).expect("v1");
    let units = document.mesh().source_units();
    assert_eq!(document::unit_names(units), ["kip", "ft", "°F"]);
    let thickness = shell(&document, "|F|L12").thickness_m.expect("declared");
    assert!(
        (thickness - 0.75 * 0.3048).abs() < 1e-12,
        "0.75 ft in metres"
    );
    assert_eq!(document::length(thickness, units), "0.2286 m (0.75 ft)");
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
            coincident: 0,
        })
        .expect("in scope");
    assert_eq!(state.model.selected(scope), [0]);
    assert_eq!(
        state.apply(Action::Pick {
            scope,
            entity: Some(999),
            toggle: false,
            coincident: 0,
        }),
        Err(Refusal::UnknownEntity)
    );
    let effect = switch(&mut state, "v1");
    assert_eq!(
        state.apply(Action::Pick {
            scope,
            entity: Some(1),
            toggle: false,
            coincident: 0,
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
            coincident: 0,
        }),
        Err(Refusal::StaleSelection),
        "a pick made against v2 is refused on v1"
    );
}

#[test]
fn identical_bytes_as_another_version_still_expire_the_selection() {
    let source = Project::public_demo().versions[0].source.clone();
    let project = Project {
        name: "Same bytes".to_owned(),
        authority: Authority::PublicSyntheticDemo,
        versions: ["a", "b"]
            .map(|name| VersionEntry {
                id: v(name),
                summary: "same bytes".to_owned(),
                source: source.clone(),
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
            coincident: 0,
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
            coincident: 0,
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
            coincident: 0,
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
        .apply(Action::SetStoryRange(range("L12", "L12")))
        .expect("valid");
    assert_eq!(
        state.apply(Action::SetStoryRange(range("Base", "L12"))),
        Err(Refusal::UnknownStory),
        "inverted"
    );
    assert_eq!(
        state.apply(Action::SetStoryRange(range("L12", "Level 99"))),
        Err(Refusal::UnknownStory)
    );
    assert_eq!(state.results.query.stories, range("L12", "L12"));
    assert_eq!(
        crate::pages::results::describe(&state.results.query),
        "Story drift · wind · X · no case · L12–L12 · main/v2"
    );
}

#[test]
fn a_story_range_missing_from_the_next_version_is_reset_with_a_notice() {
    let Source::Embedded(bytes) = Project::public_demo().versions[0].source else {
        panic!("the tower is embedded")
    };
    let renamed = String::from_utf8(bytes.to_vec())
        .expect("utf-8")
        .replace("\"L12\"", "\"Level 13\"");
    let mut project = Project::public_demo();
    project.versions[0].source = Source::Embedded(leaked(renamed));
    let (mut state, effects) = State::new(project);
    deliver(&mut state, &effects[0]).expect("v2");
    state
        .apply(Action::SetStoryRange(Some(StoryRange {
            top: "L12".to_owned(),
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
            .is_some_and(|n| n.contains("L12–Base") && n.contains("main/v1")),
        "{:?}",
        state.results.notice
    );
    // The notice describes the v1 load only; it must not follow the user back.
    let effect = switch(&mut state, "v2");
    deliver(&mut state, &effect).expect("v2");
    assert_eq!(state.results.notice, None, "stale notice about main/v1");
    assert_eq!(state.results.query.version, Some(v("v2")));
}

#[test]
fn story_highlight_is_validated_and_survives_only_where_the_story_exists() {
    let mut state = ready_demo();
    assert_eq!(
        state.apply(Action::SelectStory(Some("Level 99".to_owned()))),
        Err(Refusal::UnknownStory)
    );
    state
        .apply(Action::SelectStory(Some("L12".to_owned())))
        .expect("story of v2");
    let effect = switch(&mut state, "v1");
    deliver(&mut state, &effect).expect("v1");
    assert_eq!(
        state.model.story.as_deref(),
        Some("L12"),
        "same story in v1"
    );

    // A version without that story drops the highlight instead of keeping a
    // name that no longer resolves.
    let Source::Embedded(bytes) = Project::public_demo().versions[0].source else {
        panic!("the tower is embedded")
    };
    let renamed = String::from_utf8(bytes.to_vec())
        .expect("utf-8")
        .replace("\"L12\"", "\"Level 13\"");
    let mut project = Project::public_demo();
    project.versions[0].source = Source::Embedded(leaked(renamed));
    let (mut state, effects) = State::new(project);
    deliver(&mut state, &effects[0]).expect("v2");
    state
        .apply(Action::SelectStory(Some("L12".to_owned())))
        .expect("story of v2");
    let effect = switch(&mut state, "v1");
    deliver(&mut state, &effect).expect("v1");
    assert_eq!(state.model.story, None);
    state.apply(Action::SelectStory(None)).expect("clear");
}

#[test]
fn an_ambiguous_view_pick_is_disclosed_until_the_next_pick_or_version() {
    let mut state = ready_demo();
    let scope = state.session.scope().expect("ready");
    let pick = |entity, coincident| Action::Pick {
        scope,
        entity,
        toggle: false,
        coincident,
    };
    state.apply(pick(Some(3), 2)).expect("in scope");
    assert_eq!(state.model.ambiguous, 2);
    state.apply(pick(None, 0)).expect("empty click");
    assert_eq!(state.model.ambiguous, 0, "an empty click clears it");
    state.apply(pick(Some(3), 1)).expect("in scope");
    let _load = switch(&mut state, "v1");
    assert_eq!(state.model.ambiguous, 0, "a version switch clears it");
}

#[test]
fn gpu_geometry_keys_follow_geometry_bytes_not_version_names() {
    use lab_viewport::GpuGeometry as _;
    let project = Project::public_demo();
    let load = |entry: &VersionEntry| Document::load(entry).expect("fixture");
    let (v1, v2) = (load(&project.versions[0]), load(&project.versions[1]));
    assert_ne!(
        v1.key(),
        v2.key(),
        "a version switch uploads the new geometry"
    );
    let mut same = project.versions[1].clone();
    same.id = v("v3");
    assert_eq!(
        load(&same).key(),
        v2.key(),
        "identical geometry reuses resident buffers"
    );
    assert_eq!(v2.vertices().len(), v2.mesh().vertices().len(), "no copy");
}

#[test]
fn an_explicit_story_change_clears_the_member_selection() {
    let mut state = ready_demo();
    let scope = state.session.scope().expect("ready");
    let member = state
        .session
        .ready()
        .expect("ready")
        .mesh()
        .entities()
        .iter()
        .position(|e| e.story == "L12")
        .expect("an L12 member");
    state
        .apply(Action::Pick {
            scope,
            entity: Some(member),
            toggle: false,
            coincident: 1,
        })
        .expect("in scope");
    assert_eq!(state.model.selected(scope), [member]);
    state
        .apply(Action::SelectStory(Some("L1".to_owned())))
        .expect("story of v2");
    assert_eq!(state.model.selected(scope), [] as [usize; 0]);
    assert_eq!(state.model.ambiguous, 0);

    // Re-sending the current story is not a change and keeps a new pick.
    state
        .apply(Action::Pick {
            scope,
            entity: Some(member),
            toggle: false,
            coincident: 0,
        })
        .expect("in scope");
    state
        .apply(Action::SelectStory(Some("L1".to_owned())))
        .expect("same story");
    assert_eq!(state.model.selected(scope), [member]);
    state.apply(Action::SelectStory(None)).expect("clear");
    assert_eq!(state.model.selected(scope), [] as [usize; 0]);
}

#[test]
fn a_compact_window_shows_one_panel_and_keeps_the_panel_intent() {
    let mut state = ready_demo();
    let scope = state.session.scope().expect("ready");
    let shown = |state: &State| (state.model.shows_explorer(), state.model.shows_inspector());
    state.apply(Action::SetCompact(true)).expect("width class");
    assert_eq!(shown(&state), (true, false), "Explorer by default");
    state
        .apply(Action::Pick {
            scope,
            entity: Some(3),
            toggle: false,
            coincident: 0,
        })
        .expect("in scope");
    assert_eq!(
        shown(&state),
        (false, true),
        "a member pick shows the Inspector"
    );
    state.apply(Action::ToggleInspector).expect("I");
    assert_eq!(shown(&state), (true, false), "I swaps back to the Explorer");
    state.apply(Action::ToggleExplorer).expect("E");
    assert_eq!(shown(&state), (false, true), "E swaps to the Inspector");
    assert!(state.model.explorer_open && state.model.inspector_open);
    assert_eq!(
        state.model.selected(scope),
        [3],
        "swapping never drops the pick"
    );

    // Widening restores both, with the same selection and no reload.
    state.apply(Action::SetCompact(false)).expect("width class");
    assert_eq!(shown(&state), (true, true));
    assert_eq!(state.session.scope(), Some(scope));
    assert_eq!(state.model.selected(scope), [3]);

    // An Inspector the user closed stays closed when a pick happens compact.
    state
        .apply(Action::ToggleInspector)
        .expect("I, wide: close");
    state.apply(Action::SetCompact(true)).expect("width class");
    state
        .apply(Action::Pick {
            scope,
            entity: Some(4),
            toggle: false,
            coincident: 0,
        })
        .expect("in scope");
    assert_eq!(shown(&state), (true, false));
    state
        .apply(Action::ToggleInspector)
        .expect("I, compact: open");
    assert_eq!(
        shown(&state),
        (false, true),
        "an explicit toggle always works"
    );

    // Clearing the selection or switching versions hands the context back.
    state
        .apply(Action::Pick {
            scope,
            entity: None,
            toggle: false,
            coincident: 0,
        })
        .expect("empty click");
    assert_eq!(shown(&state), (true, false));
    state
        .apply(Action::Pick {
            scope,
            entity: Some(4),
            toggle: false,
            coincident: 0,
        })
        .expect("in scope");
    let _load = switch(&mut state, "v1");
    assert_eq!(state.model.context, crate::state::Context::Explorer);
}

#[test]
fn show_in_explorer_moves_the_filter_to_the_selection_and_keeps_it() {
    let mut state = ready_demo();
    let scope = state.session.scope().expect("ready");
    let entities = state
        .session
        .ready()
        .expect("ready")
        .mesh()
        .entities()
        .to_vec();
    let on = |story: &str| {
        entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.story == story)
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    };
    let (l1, l2) = (on("L1"), on("L2"));
    state
        .apply(Action::SelectStory(Some("L5".to_owned())))
        .expect("story");
    let pick = |entity, toggle| Action::Pick {
        scope,
        entity: Some(entity),
        toggle,
        coincident: 0,
    };
    state
        .apply(pick(l1[0], false))
        .expect("a pick on another story");
    assert_eq!(
        state.model.story.as_deref(),
        Some("L5"),
        "picking keeps the filter"
    );
    state.apply(Action::RevealSelection).expect("reveal");
    assert_eq!(state.model.story.as_deref(), Some("L1"));
    assert_eq!(
        state.model.selected(scope),
        [l1[0]],
        "the selection is kept"
    );
    // A selection across stories clears the filter instead.
    state.apply(pick(l2[0], true)).expect("ctrl-add");
    state.apply(Action::RevealSelection).expect("reveal");
    assert_eq!(state.model.story, None);
    assert_eq!(state.model.selected(scope).len(), 2);
}
