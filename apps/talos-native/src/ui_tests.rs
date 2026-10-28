//! In-process tests of the real application UI through egui's AccessKit tree;
//! no native window, GPU, `WebView` or desktop input.
use crate::app::App;
use crate::loader::{LoadMode, Loader};
use crate::project::{Project, VersionId};
use crate::state::{DocumentState, Metric, Refusal, Workspace};
use crate::view::PlaceholderView;
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};

fn harness(mode: LoadMode) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        // A loading spinner animates; a settled UI needs far fewer steps.
        .with_max_steps(8)
        .with_size(egui::vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            crate::theme::install(&cc.egui_ctx);
            App::with_parts(
                Project::public_demo(),
                Box::new(PlaceholderView),
                Loader::new(mode, Some(cc.egui_ctx.clone())),
            )
        });
    settle(&mut harness);
    harness
}
/// Runs until idle, or for a bounded number of frames while a spinner animates.
fn settle(harness: &mut Harness<'_, App>) {
    harness.run_ok();
}
fn selected_tabs(harness: &Harness<'_, App>) -> Vec<String> {
    harness
        .get_all_by_role(Role::Tab)
        .filter(|tab| tab.accesskit_node().is_selected() == Some(true))
        .filter_map(|tab| tab.accesskit_node().label())
        .collect()
}
fn present(harness: &Harness<'_, App>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}
/// The explorer row for shell `S`.
fn slab_row<'a>(harness: &'a Harness<'_, App>) -> egui_kittest::Node<'a> {
    harness
        .get_all_by_role(Role::Button)
        .find(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.starts_with("S  ·  Floor shell"))
        })
        .expect("explorer row for slab S")
}
fn choose_version(harness: &mut Harness<'_, App>, version: &str) {
    harness
        .get_by_role_and_label(Role::ComboBox, "Version")
        .click();
    settle(harness);
    harness
        .get_by_label_contains(&format!("{version}   "))
        .click();
    settle(harness);
}

#[test]
fn launches_as_a_tab_list_on_model_with_a_restrained_header() {
    let harness = harness(LoadMode::Inline);
    let list = harness.get_by_role(Role::TabList);
    assert_eq!(list.accesskit_node().label().as_deref(), Some("Workspaces"));
    let tabs: Vec<_> = harness
        .get_all_by_role(Role::Tab)
        .filter_map(|tab| tab.accesskit_node().label())
        .collect();
    assert_eq!(tabs, ["Model", "Results", "Compare", "History", "Reports"]);
    assert_eq!(selected_tabs(&harness), ["Model"]);
    for text in [
        "Public demo project",
        "Synthetic demo",
        "Read-only",
        "ETABS not connected",
    ] {
        assert!(present(&harness, text), "{text}");
    }
    assert!(present(&harness, "3D viewport"));
    assert!(
        present(&harness, "Source units kip · ft · °F"),
        "units live in Model"
    );
    assert!(
        !present(&harness, "No analyzed results"),
        "Results is not co-rendered"
    );
}

#[test]
fn model_and_results_never_render_together() {
    let mut harness = harness(LoadMode::Inline);
    harness.get_by_label("Results").click();
    harness.run();
    assert_eq!(selected_tabs(&harness), ["Results"]);
    assert_eq!(harness.state().state().workspace, Workspace::Results);
    assert!(present(&harness, "No analyzed results for main/v2"));
    for absent in ["3D viewport", "Explorer", "Inspector", "Source units"] {
        assert!(!present(&harness, absent), "{absent} leaked into Results");
    }
    harness.get_by_label("Compare").click();
    harness.run();
    assert!(present(&harness, "Not available in this prototype yet"));
    assert!(!present(&harness, "No analyzed results"));
    assert!(!present(&harness, "3D viewport"));
}

#[test]
fn keyboard_shortcuts_and_arrow_keys_route_workspaces() {
    let mut harness = harness(LoadMode::Inline);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    harness.run();
    assert_eq!(selected_tabs(&harness), ["Results"]);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::PageDown);
    harness.run();
    assert_eq!(selected_tabs(&harness), ["Compare"]);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    harness.run();
    assert_eq!(selected_tabs(&harness), ["Model"]);
    harness.get_by_label("Model").focus();
    harness.run();
    harness.key_press(egui::Key::ArrowLeft);
    harness.run();
    assert_eq!(selected_tabs(&harness), ["Reports"], "Left wraps");
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    assert_eq!(selected_tabs(&harness), ["Results"]);
    assert!(
        harness
            .get_by_label("Results")
            .accesskit_node()
            .is_focused(),
        "focus follows the active tab"
    );
}

#[test]
fn workspace_state_survives_tab_switches() {
    let mut harness = harness(LoadMode::Inline);
    harness.key_press(egui::Key::E);
    harness.run();
    assert!(!harness.state().state().model.explorer_open);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    harness.run();
    harness.get_by_label("Metric").click();
    harness.run();
    harness.get_by_label("Story shear").click();
    harness.run();
    harness.key_press(egui::Key::E);
    harness.run();
    assert!(
        !harness.state().state().model.explorer_open,
        "E belongs to Model only"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    harness.run();
    assert!(
        !present(&harness, "Source entities"),
        "explorer stays closed"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    harness.run();
    assert_eq!(
        harness.state().state().results.query.metric,
        Metric::StoryShear
    );
    assert!(present(
        &harness,
        "Story shear · X · no case · all stories · main/v2"
    ));
}

#[test]
fn explorer_selection_drives_the_inspector_and_a_version_switch_clears_it() {
    let mut harness = harness(LoadMode::Inline);
    assert!(present(&harness, "Nothing selected"));
    slab_row(&harness).click();
    harness.run();
    assert!(present(&harness, "FlatAtDeclaredFace"), "v2 is TOP");
    assert!(present(&harness, "0.1524 m (0.5 ft)"));
    choose_version(&mut harness, "v1");
    assert_eq!(
        harness.state().state().session.requested(),
        &VersionId::new("main", "v1")
    );
    assert!(present(&harness, "Nothing selected"), "selection expired");
    slab_row(&harness).click();
    harness.run();
    assert!(present(&harness, "CentredMiddle"), "v1 is MIDDLE");
    assert!(!present(&harness, "FlatAtDeclaredFace"));
}

#[test]
fn rapid_switches_show_only_the_current_version_whatever_order_loads_finish() {
    let mut harness = harness(LoadMode::Manual);
    assert!(present(&harness, "Loading main/v2"));
    assert!(
        !present(&harness, "Source entities"),
        "no content before a load"
    );
    choose_version(&mut harness, "v1");
    choose_version(&mut harness, "v2");
    assert_eq!(harness.state_mut().loader_mut().held().len(), 3);
    // Held: [g1 v2, g2 v1, g3 v2]. The oldest v2 finishes while g3 is still
    // pending, then the current one, then the stale v1.
    for index in [0, 1, 0] {
        harness.state_mut().loader_mut().release(index);
        settle(&mut harness);
    }
    let app = harness.state();
    let DocumentState::Ready { ticket, document } = &app.state().session.document else {
        panic!("ready");
    };
    assert_eq!(
        (ticket.generation, document.version().version.as_str()),
        (3, "v2")
    );
    assert_eq!(app.state().stale_discarded, 2);
    assert_eq!(app.last_refusal(), Some(Refusal::StaleResponse));
    assert!(present(&harness, "Source entities"));
}
