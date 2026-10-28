//! In-process tests of the real application UI through egui's AccessKit tree;
//! no native window, `WebView` or desktop input. The model view is the real
//! `NativeModelView` without a GPU (picking is CPU work); the opt-in GPU tests
//! in `gpu_tests` add a real wgpu renderer.
use crate::app::App;
use crate::loader::{LoadMode, Loader};
use crate::native_view::NativeModelView;
use crate::project::{Project, VersionId};
use crate::state::Action;
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
                Box::new(NativeModelView::new(None)),
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
/// Index of the entity whose stable key contains `needle`, in the loaded version.
fn entity(harness: &Harness<'_, App>, needle: &str) -> usize {
    let document = harness.state().state().session.ready().expect("ready");
    document
        .mesh()
        .entities()
        .iter()
        .position(|e| e.key.to_string().contains(needle))
        .expect("entity in the fixture")
}
/// Selects through the shell's own action, as the explorer does; for rows the
/// virtualized explorer has not laid out.
fn select(harness: &mut Harness<'_, App>, needle: &str) {
    let index = entity(harness, needle);
    let scope = harness.state().state().session.scope().expect("ready");
    harness.state_mut().dispatch(Action::Pick {
        scope,
        entity: Some(index),
        toggle: false,
        coincident: 0,
    });
    settle(harness);
}
fn row<'a>(harness: &'a Harness<'_, App>, prefix: &str) -> egui_kittest::Node<'a> {
    harness
        .get_all_by_role(Role::Button)
        .find(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.starts_with(prefix))
        })
        .unwrap_or_else(|| panic!("button starting with {prefix:?}"))
}
fn view<'a>(harness: &'a Harness<'_, App>) -> &'a NativeModelView {
    harness
        .state()
        .view()
        .as_any()
        .downcast_ref::<NativeModelView>()
        .expect("native view")
}
/// Screen position of the centroid of an entity's drawn triangles, through the
/// view's current camera, and that centroid's depth along the camera ray.
fn screen_point(harness: &Harness<'_, App>, index: usize) -> egui::Pos2 {
    let document = harness.state().state().session.ready().expect("ready");
    let mesh = document.mesh();
    let range = &mesh.entities()[index].indices;
    let ids = &mesh.indices()[range.start as usize..range.end as usize];
    let mut sum = [0.0_f32; 3];
    for &id in ids {
        let p = mesh.vertices()[id as usize].position;
        for axis in 0..3 {
            sum[axis] += p[axis];
        }
    }
    let n = ids.len() as f32;
    let centroid = sum.map(|v| v / n);
    let rect = harness.get_by_label("3D model view").rect();
    let camera = view(harness).camera().expect("camera after first frame");
    let m = camera.view_projection(rect.width() / rect.height());
    let clip: [f32; 4] = std::array::from_fn(|row| {
        (0..3).map(|col| m[col][row] * centroid[col]).sum::<f32>() + m[3][row]
    });
    egui::pos2(
        rect.left() + (clip[0] / clip[3] * 0.5 + 0.5) * rect.width(),
        rect.top() + (0.5 - clip[1] / clip[3] * 0.5) * rect.height(),
    )
}
fn click_at(harness: &mut Harness<'_, App>, position: egui::Pos2, modifiers: egui::Modifiers) {
    harness.hover_at(position);
    // egui reads click modifiers from its input state, as a keyboard sets it.
    harness.event(egui::Event::ModifiersChanged(modifiers));
    harness.event(egui::Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers,
    });
    harness.event(egui::Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers,
    });
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    settle(harness);
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
        "Public demo tower",
        "Synthetic demo",
        "Read-only",
        "ETABS not connected",
    ] {
        assert!(present(&harness, text), "{text}");
    }
    assert!(present(&harness, "3D model view"));
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
    for absent in ["3D model view", "Explorer", "Inspector", "Source units"] {
        assert!(!present(&harness, absent), "{absent} leaked into Results");
    }
    harness.get_by_label("Compare").click();
    harness.run();
    assert!(present(&harness, "Not available in this prototype yet"));
    assert!(!present(&harness, "No analyzed results"));
    assert!(!present(&harness, "3D model view"));
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
    assert!(!present(&harness, "All entities"), "explorer stays closed");
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
fn explorer_story_and_row_drive_highlight_and_inspector_and_a_switch_clears_selection() {
    let mut harness = harness(LoadMode::Inline);
    assert!(present(&harness, "Nothing selected"));
    assert!(present(&harness, "All entities (672)"));
    row(&harness, "L12  ·  56").click();
    settle(&mut harness);
    assert_eq!(harness.state().state().model.story.as_deref(), Some("L12"));
    assert!(present(&harness, "Entities on L12 (56)"));
    row(&harness, "C0_0  ·  Column  ·  L12").click();
    settle(&mut harness);
    assert!(present(&harness, "0.6096 m (2 ft)"), "C24 above L4, v2");
    select(&mut harness, "|F|L12");
    assert!(
        present(&harness, "FlatAtDeclaredFace"),
        "v2 roof slab is TOP"
    );
    assert!(present(&harness, "0.2286 m (0.75 ft)"));
    choose_version(&mut harness, "v1");
    assert_eq!(
        harness.state().state().session.requested(),
        &VersionId::new("main", "v1")
    );
    assert!(present(&harness, "Nothing selected"), "selection expired");
    assert_eq!(
        harness.state().state().model.story.as_deref(),
        Some("L12"),
        "the story exists in v1 too"
    );
    select(&mut harness, "|F|L12");
    assert!(present(&harness, "CentredMiddle"), "v1 roof slab is MIDDLE");
    assert!(!present(&harness, "FlatAtDeclaredFace"));
}

#[test]
fn clicking_the_model_picks_the_drawn_entity_and_ctrl_click_adds_one() {
    let mut harness = harness(LoadMode::Inline);
    let slab = entity(&harness, "|F|L12");
    // A slab-only point of the roof (between grid lines), seen from above.
    let target = {
        let document = harness.state().state().session.ready().expect("ready");
        let mesh = document.mesh();
        let origin = mesh.origin_m();
        let roof = mesh
            .stories()
            .iter()
            .find(|s| s.name == "L12")
            .expect("L12")
            .elevation_m;
        [
            45.0 * 0.3048 - origin[0],
            12.5 * 0.3048 - origin[1],
            roof - origin[2],
        ]
        .map(|v| v as f32)
    };
    let rect = harness.get_by_label("3D model view").rect();
    let camera = view(&harness).camera().expect("camera");
    let m = camera.view_projection(rect.width() / rect.height());
    let clip: [f32; 4] = std::array::from_fn(|row| {
        (0..3).map(|col| m[col][row] * target[col]).sum::<f32>() + m[3][row]
    });
    let at = egui::pos2(
        rect.left() + (clip[0] / clip[3] * 0.5 + 0.5) * rect.width(),
        rect.top() + (0.5 - clip[1] / clip[3] * 0.5) * rect.height(),
    );
    click_at(&mut harness, at, egui::Modifiers::NONE);
    let scope = harness.state().state().session.scope().expect("ready");
    assert_eq!(harness.state().state().model.selected(scope), [slab]);
    assert!(
        present(&harness, "FlatAtDeclaredFace"),
        "inspector follows the pick"
    );

    let column = entity(&harness, "|C0_0|L6");
    let at = screen_point(&harness, column);
    click_at(&mut harness, at, egui::Modifiers::COMMAND);
    let mut expected = vec![slab, column];
    expected.sort_unstable();
    assert_eq!(harness.state().state().model.selected(scope), expected);
    assert!(present(&harness, "2 entities selected"));

    // Empty space (a corner of the canvas) clears without toggle.
    click_at(
        &mut harness,
        rect.left_top() + egui::vec2(12.0, 12.0),
        egui::Modifiers::NONE,
    );
    assert!(harness.state().state().model.selected(scope).is_empty());
}

#[test]
fn a_version_switch_keeps_the_view_and_clears_the_pick() {
    let mut harness = harness(LoadMode::Inline);
    harness.get_by_label("3D model view").hover();
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 120.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut harness);
    let before = view(&harness).camera().expect("camera");
    select(&mut harness, "|F|L12");
    choose_version(&mut harness, "v1");
    let after = view(&harness).camera().expect("camera");
    assert_eq!((after.yaw, after.pitch), (before.yaw, before.pitch));
    assert!(
        (after.distance / after.radius - before.distance / before.radius).abs() < 1e-3,
        "relative zoom kept across versions"
    );
    let scope = harness.state().state().session.scope().expect("ready");
    assert!(harness.state().state().model.selected(scope).is_empty());
    harness.key_press(egui::Key::F);
    settle(&mut harness);
    let fitted = view(&harness).camera().expect("camera");
    assert!(
        fitted.distance > after.distance,
        "F refits after zooming in"
    );
}

#[test]
fn rapid_switches_show_only_the_current_version_whatever_order_loads_finish() {
    let mut harness = harness(LoadMode::Manual);
    assert!(present(&harness, "Loading main/v2"));
    assert!(
        !present(&harness, "All entities"),
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
    assert!(present(&harness, "All entities"));
}

/// Paint bounds (logical points) of every text label under `root`.
fn label_rects<'a>(
    nodes: impl Iterator<Item = egui_kittest::Node<'a>>,
) -> Vec<(String, egui::Rect)> {
    nodes
        .filter(|node| node.accesskit_node().role() == Role::Label)
        .map(|node| {
            let text = node.accesskit_node().value().unwrap_or_default();
            (text, node.rect())
        })
        .collect()
}
fn assert_within(labels: &[(String, egui::Rect)], left: f32, right: f32, context: &str) {
    assert!(!labels.is_empty(), "{context}: no labels");
    for (text, rect) in labels {
        assert!(
            rect.min.x >= left - 0.5 && rect.max.x <= right + 0.5,
            "{context}: {text:?} at {rect:?} leaves [{left}, {right}]"
        );
    }
}

#[test]
fn inspector_text_stays_inside_its_panel_in_the_app() {
    let mut harness = harness(LoadMode::Inline);
    for version in ["v2", "v1"] {
        if version == "v1" {
            choose_version(&mut harness, "v1");
        }
        select(&mut harness, "|F|L12");
        // The inspector's own pane: the smallest pane holding its heading.
        let pane = harness
            .root()
            .children_recursive()
            .filter(|node| node.accesskit_node().role() == Role::Pane)
            .filter(|pane| {
                pane.children_recursive()
                    .any(|n| n.accesskit_node().value().as_deref() == Some("Inspector"))
            })
            .min_by_key(|pane| pane.children_recursive().count())
            .expect("inspector pane");
        let labels = label_rects(pane.children_recursive());
        let left = harness
            .get_all_by_role(Role::Splitter)
            .map(|splitter| splitter.rect().center().x)
            .fold(f32::MIN, f32::max);
        assert_within(&labels, left, 1280.0, version);
        let expected = if version == "v2" {
            "FlatAtDeclaredFace"
        } else {
            "CentredMiddle"
        };
        assert!(labels.iter().any(|(text, _)| text == expected), "{version}");
    }
}

/// `tower-v1.e2k` with slab `F` renamed to one long unbroken identifier.
fn long_name_version() -> crate::project::VersionEntry {
    let mut entry = Project::public_demo().versions[0].clone();
    let crate::project::Source::Embedded(bytes) = entry.source;
    let text = String::from_utf8(bytes.to_vec())
        .expect("utf-8")
        .replace("AREA \"F\"", &format!("AREA \"{LONG_NAME}\""))
        .replace("AREAASSIGN \"F\"", &format!("AREAASSIGN \"{LONG_NAME}\""));
    entry.source =
        crate::project::Source::Embedded(Box::leak(text.into_bytes().into_boxed_slice()));
    entry
}
const LONG_NAME: &str =
    "S_DELIBERATELY_LONG_SOURCE_IDENTIFIER_0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// One entity of each class plus every roof (L12) shell: the inspector's
/// distinct row shapes, including the TOP slab and the long identifier.
fn sample(document: &crate::document::Document) -> Vec<usize> {
    let mut seen = Vec::new();
    let mut picked = Vec::new();
    for (index, entity) in document.mesh().entities().iter().enumerate() {
        let roof_shell = entity.shell.is_some() && entity.story == "L12";
        if roof_shell || !seen.contains(&entity.class) {
            seen.push(entity.class);
            picked.push(index);
        }
    }
    picked
}

#[test]
fn inspector_rows_fit_every_width_scale_and_theme_without_eliding() {
    let project = Project::public_demo();
    let documents: Vec<_> = [
        project.versions[0].clone(),
        project.versions[1].clone(),
        long_name_version(),
    ]
    .iter()
    .map(|entry| std::sync::Arc::new(crate::document::Document::load(entry).expect("fixture")))
    .collect();
    for width in [240.0, 320.0, 520.0] {
        for scale in [1.0, 1.25, 1.5] {
            for theme in [egui::Theme::Dark, egui::Theme::Light] {
                for document in &documents {
                    for index in sample(document) {
                        let shown = std::sync::Arc::clone(document);
                        let harness = Harness::builder()
                            .with_size(egui::vec2(width, 900.0))
                            .with_pixels_per_point(scale)
                            .with_theme(theme)
                            .build_ui(move |ui| {
                                crate::pages::model::inspector(ui, &shown, &[index], 0);
                            });
                        let context = format!(
                            "width {width} scale {scale} {theme:?} {} entity {index}",
                            document.version()
                        );
                        let labels = label_rects(harness.root().children_recursive());
                        assert_within(&labels, 0.0, width, &context);
                        // Full values, not elided: the source key and, for shells,
                        // the native-display sentence appear whole.
                        let entity = &document.mesh().entities()[index];
                        let mut whole = vec![entity.key.to_string()];
                        if let Some(shell) = &entity.shell {
                            whole.push(shell.placement.native_display().to_owned());
                        }
                        for text in whole {
                            assert!(
                                labels.iter().any(|(label, _)| *label == text),
                                "{context}: {text:?} missing or cut"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(
        documents[2]
            .mesh()
            .entities()
            .iter()
            .any(|e| e.key.to_string().contains(LONG_NAME)),
        "the long identifier reached the inspector"
    );
}

#[test]
fn no_empty_warning_band_when_nothing_is_wrong() {
    let harness = harness(LoadMode::Inline);
    let version = harness
        .get_by_role_and_label(Role::ComboBox, "Version")
        .rect();
    let tabs = harness.get_by_label("Model").rect();
    let gap = tabs.min.y - version.max.y;
    assert!(gap < 12.0, "{gap} pt between the header and the tabs");
}

#[test]
fn a_failed_load_still_shows_its_warning() {
    let mut project = Project::public_demo();
    project.versions[1].source = crate::project::Source::Embedded(b"not an E2K file");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_max_steps(8)
        .build_eframe(move |cc| {
            App::with_parts(
                project,
                Box::new(PlaceholderView),
                Loader::new(LoadMode::Inline, Some(cc.egui_ctx.clone())),
            )
        });
    settle(&mut harness);
    assert!(present(&harness, "Could not load main/v2"));
    assert!(present(&harness, "main/v2 is not available"));
}

#[test]
fn a_hidden_model_tab_lays_out_no_view_and_keeps_the_same_view() {
    let mut harness = harness(LoadMode::Inline);
    let shown = view(&harness).frames();
    assert!(shown > 0);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    let hidden = view(&harness).frames();
    for _ in 0..5 {
        harness.step();
    }
    assert_eq!(
        view(&harness).frames(),
        hidden,
        "no view work while Results is active"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    settle(&mut harness);
    assert!(view(&harness).frames() > hidden, "the same view resumes");
}
