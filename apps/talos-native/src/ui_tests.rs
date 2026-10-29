//! In-process tests of the real application UI through egui's AccessKit tree;
//! no native window, `WebView` or desktop input. The model view is the real
//! `NativeModelView` without a GPU (picking is CPU work); the opt-in GPU tests
//! in `gpu_tests` add a real wgpu renderer.
use crate::app::App;
use crate::loader::{LoadMode, Loader};
use crate::native_view::NativeModelView;
use crate::project::{Project, VersionId};
use crate::state::Action;
use crate::state::{DocumentState, PanelMode, Refusal, Workspace};
use crate::view::PlaceholderView;
use egui::accesskit::{Role, Toggled};
use egui_kittest::{
    Harness,
    kittest::{NodeT as _, Queryable as _},
};

fn harness(mode: LoadMode) -> Harness<'static, App> {
    harness_sized(mode, egui::vec2(1280.0, 800.0))
}
fn harness_sized(mode: LoadMode, size: egui::Vec2) -> Harness<'static, App> {
    harness_scaled(mode, size, 1.0)
}
fn harness_scaled(mode: LoadMode, size: egui::Vec2, scale: f32) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        // A loading spinner animates; a settled UI needs far fewer steps.
        .with_max_steps(8)
        .with_size(size)
        .with_pixels_per_point(scale)
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
        !present(&harness, "No cached results"),
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
    assert!(present(&harness, "No cached results for main/v2"));
    for absent in ["3D model view", "Explorer", "Inspector", "Source units"] {
        assert!(!present(&harness, absent), "{absent} leaked into Results");
    }
    harness.get_by_label("Compare").click();
    harness.run();
    assert!(present(&harness, "Not available in this prototype yet"));
    assert!(!present(&harness, "No cached results"));
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
    // The tower has no cached results, so the Metric list is disabled; the
    // direction is still part of the request and must survive tab switches.
    harness.get_by_label("Y").click();
    harness.run();
    harness.key_press(egui::Key::E);
    harness.run();
    assert!(
        !harness.state().state().model.explorer_open,
        "E belongs to Model only"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    harness.run();
    assert!(!present(&harness, "ALL ENTITIES"), "explorer stays closed");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    harness.run();
    assert_eq!(
        harness.state().state().results.query.direction,
        crate::state::Direction::Y
    );
    assert!(present(
        &harness,
        "Story drift · wind · Y · no case · all stories · main/v2"
    ));
}

#[test]
fn explorer_story_and_row_drive_highlight_and_inspector_and_a_switch_clears_selection() {
    let mut harness = harness(LoadMode::Inline);
    assert!(present(&harness, "Nothing selected"));
    assert!(present(&harness, "ALL ENTITIES (672)"));
    row(&harness, "L12  ·  56").click();
    settle(&mut harness);
    assert_eq!(harness.state().state().model.story.as_deref(), Some("L12"));
    assert!(present(&harness, "ENTITIES ON L12 (56)"));
    row(&harness, "C0_0  ·  Column  ·  L12").click();
    settle(&mut harness);
    assert!(present(&harness, "0.6096 m (2 ft)"), "C24 above L4, v2");
    select(&mut harness, "|F|L12");
    assert!(
        present(&harness, "flat at declared face"),
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
    assert!(
        present(&harness, "centred middle"),
        "v1 roof slab is MIDDLE"
    );
    assert!(!present(&harness, "flat at declared face"));
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
        .map(|v| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a local display coordinate, as the renderer's f32 vertices"
            )]
            let local = v as f32;
            local
        })
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
        present(&harness, "flat at declared face"),
        "inspector follows the pick"
    );

    let column = entity(&harness, "|C0_0|L6");
    let at = screen_point(&harness, column);
    click_at(&mut harness, at, egui::Modifiers::COMMAND);
    let mut expected = vec![slab, column];
    expected.sort_unstable();
    assert_eq!(harness.state().state().model.selected(scope), expected);
    assert!(present(&harness, "2 entities selected"));

    // Empty space (the canvas's left edge, clear of every overlay) clears
    // without toggle.
    click_at(
        &mut harness,
        rect.left_center() + egui::vec2(6.0, 0.0),
        egui::Modifiers::NONE,
    );
    assert_eq!(
        harness.state().state().model.selected(scope),
        [] as [usize; 0]
    );
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
    assert_eq!(
        harness.state().state().model.selected(scope),
        [] as [usize; 0]
    );
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
        !present(&harness, "ALL ENTITIES"),
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
    assert!(present(&harness, "ALL ENTITIES"));
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

/// The inspector's own pane or card: the smallest pane holding its close button.
fn inspector_labels(harness: &Harness<'_, App>) -> Vec<(String, egui::Rect)> {
    let pane = harness
        .root()
        .children_recursive()
        .filter(|node| node.accesskit_node().role() == Role::Pane)
        .filter(|pane| {
            pane.children_recursive()
                .any(|n| n.accesskit_node().label().as_deref() == Some("Hide inspector (I)"))
        })
        .min_by_key(|pane| pane.children_recursive().count())
        .expect("inspector pane");
    label_rects(pane.children_recursive())
}

#[test]
fn inspector_text_stays_inside_its_panel_in_the_app() {
    let mut harness = harness(LoadMode::Inline);
    for version in ["v2", "v1"] {
        if version == "v1" {
            choose_version(&mut harness, "v1");
        }
        select(&mut harness, "|F|L12");
        let expected = if version == "v2" {
            "flat at declared face"
        } else {
            "centred middle"
        };
        // Floating (the default): inside the card, inside the canvas margin.
        let canvas = harness.get_by_label("3D model view").rect();
        let right = canvas.right() - crate::pages::model::MARGIN;
        let left = right - crate::pages::model::pane_widths(canvas.width()).1;
        let labels = inspector_labels(&harness);
        assert_within(&labels, left, right, &format!("{version} floating"));
        assert!(labels.iter().any(|(text, _)| text == expected), "{version}");
        // Docked: between the panel's splitter and the window edge.
        set_mode(&mut harness, "Docked panels");
        let labels = inspector_labels(&harness);
        let left = harness
            .get_all_by_role(Role::Splitter)
            .map(|splitter| splitter.rect().center().x)
            .fold(f32::MIN, f32::max);
        assert_within(&labels, left, 1280.0, &format!("{version} docked"));
        assert!(labels.iter().any(|(text, _)| text == expected), "{version}");
        set_mode(&mut harness, "Floating panels");
    }
}

/// `tower-v1.e2k` with slab `F` renamed to one long unbroken identifier.
fn long_name_version() -> crate::project::VersionEntry {
    let mut entry = Project::public_demo().versions[0].clone();
    let crate::project::Source::Embedded(bytes) = entry.source else {
        panic!("the tower is embedded")
    };
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
                            .with_context_setup(crate::theme::install)
                            .build_ui(move |ui| {
                                crate::pages::model::inspector(ui, &shown, &[index], 0, false);
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
fn the_header_is_one_48_point_row_with_no_empty_warning_band() {
    let harness = harness(LoadMode::Inline);
    let version = harness
        .get_by_role_and_label(Role::ComboBox, "Version")
        .rect();
    let tab = harness.get_by_label("Model").rect();
    assert!(
        (version.center().y - tab.center().y).abs() < 1.0,
        "the version picker and the tabs share one row"
    );
    assert!(tab.height() <= 48.5 && tab.bottom() <= 49.5, "{tab:?}");
    let canvas = harness.get_by_label("3D model view").rect();
    assert!(
        (48.0..=52.0).contains(&canvas.top()),
        "the canvas starts right under the header: {canvas:?}"
    );
}

#[test]
fn a_failed_load_still_shows_its_warning() {
    let mut project = Project::public_demo();
    project.versions[1].source = crate::project::Source::Embedded(b"not an E2K file");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_max_steps(8)
        .with_context_setup(crate::theme::install)
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

fn selected(harness: &Harness<'_, App>) -> Vec<usize> {
    let state = harness.state().state();
    let scope = state.session.scope().expect("ready");
    state.model.selected(scope).to_vec()
}

fn set_mode(harness: &mut Harness<'_, App>, toggle: &str) {
    harness.get_by_label(toggle).click();
    settle(harness);
}

#[test]
fn panel_modes_keep_the_selection_and_overlays_never_pick_through() {
    let mut harness = harness(LoadMode::Inline);
    let toggled = |harness: &Harness<'_, App>, label: &str| {
        harness.get_by_label(label).accesskit_node().toggled() == Some(Toggled::True)
    };
    // Floating is the default: cards over an edge-to-edge canvas.
    assert_eq!(harness.state().state().model.panels, PanelMode::Floating);
    assert!(toggled(&harness, "Floating panels"));
    assert!(!toggled(&harness, "Docked panels"));
    let canvas = harness.get_by_label("3D model view").rect();
    assert!(canvas.width() > 1270.0, "{canvas:?}");

    select(&mut harness, "|F|L12");
    let slab = selected(&harness);
    assert_eq!(slab.len(), 1);
    // Empty card space and the version strip are not pick targets: a click
    // there would otherwise pick or clear.
    let close = harness.get_by_label("Hide inspector (I)").rect();
    let strip = harness.get_by_label("Version v1").rect();
    for at in [
        egui::pos2(close.center().x - 120.0, canvas.bottom() - 110.0),
        egui::pos2(canvas.right() - 60.0, strip.center().y),
    ] {
        click_at(&mut harness, at, egui::Modifiers::NONE);
        assert_eq!(selected(&harness), slab, "a click at {at:?} went through");
    }
    // Outside every card the canvas still picks.
    let column = entity(&harness, "|C0_0|L6");
    let at = screen_point(&harness, column);
    let (width, inspector) = crate::pages::model::pane_widths(canvas.width());
    assert!(
        at.x > canvas.left() + width + 20.0 && at.x < canvas.right() - inspector - 20.0,
        "{at:?} is clear of both cards"
    );
    click_at(&mut harness, at, egui::Modifiers::NONE);
    assert_eq!(selected(&harness), [column]);

    // Docked keeps the selection and the content; the strip still blocks.
    set_mode(&mut harness, "Docked panels");
    assert_eq!(harness.state().state().model.panels, PanelMode::Docked);
    assert!(toggled(&harness, "Docked panels"));
    assert_eq!(
        selected(&harness),
        [column],
        "the mode is presentation only"
    );
    assert!(present(&harness, "ALL ENTITIES (672)"));
    let docked = harness.get_by_label("3D model view").rect();
    assert!(docked.width() < canvas.width() - 400.0, "{docked:?}");
    let strip = harness.get_by_label("Version v1").rect();
    click_at(
        &mut harness,
        egui::pos2(docked.right() - 40.0, strip.center().y),
        egui::Modifiers::NONE,
    );
    assert_eq!(selected(&harness), [column]);
    set_mode(&mut harness, "Floating panels");
    assert_eq!(selected(&harness), [column]);
}

#[test]
fn the_version_strip_and_the_header_picker_are_one_controller() {
    let mut harness = harness(LoadMode::Inline);
    let current = |harness: &Harness<'_, App>, label: &str| {
        harness
            .get_by_role_and_label(Role::Button, label)
            .accesskit_node()
            .toggled()
            == Some(Toggled::True)
    };
    assert!(current(&harness, "Version v2"));
    assert!(!current(&harness, "Version v1"));
    harness
        .get_by_role_and_label(Role::Button, "Version v1")
        .click();
    settle(&mut harness);
    assert_eq!(
        harness.state().state().session.requested(),
        &VersionId::new("main", "v1")
    );
    assert!(current(&harness, "Version v1"));
    assert!(present(&harness, "v1 · viewing"));
    choose_version(&mut harness, "v2");
    assert!(current(&harness, "Version v2"));
    assert!(!current(&harness, "Version v1"));
    assert!(
        present(&harness, "no commit history, dates or analysis runs"),
        "the strip says what the snapshots are not"
    );
}

#[test]
fn the_explorer_filters_by_text_and_by_type() {
    let mut harness = harness(LoadMode::Inline);
    let expected = {
        let document = harness.state().state().session.ready().expect("ready");
        document
            .mesh()
            .entities()
            .iter()
            .filter(|e| e.key.to_string().contains("|C0_0|"))
            .count()
    };
    assert!(expected > 1);
    harness.get_by_role(Role::TextInput).focus();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("C0_0");
    settle(&mut harness);
    assert!(
        present(&harness, &format!("ALL ENTITIES ({expected})")),
        "the name filter"
    );
    for _ in 0.."C0_0".len() {
        harness.key_press(egui::Key::Backspace);
    }
    settle(&mut harness);
    assert!(present(&harness, "ALL ENTITIES (672)"));
    row(&harness, "Beams  ·  372").click();
    settle(&mut harness);
    assert!(present(&harness, "ALL ENTITIES (372)"), "the type filter");
    row(&harness, "Beams  ·  372").click();
    settle(&mut harness);
    assert!(
        present(&harness, "ALL ENTITIES (672)"),
        "clicking again clears it"
    );
    assert_eq!(selected(&harness), [] as [usize; 0], "filters never select");
}

#[test]
fn at_900_points_the_header_overflows_into_more_and_shortcuts_still_route() {
    let mut harness = harness_sized(LoadMode::Inline, egui::vec2(900.0, 700.0));
    let tabs: Vec<_> = harness
        .get_all_by_role(Role::Tab)
        .filter_map(|tab| tab.accesskit_node().label())
        .collect();
    assert_eq!(tabs, ["Model", "Results"]);
    let more = harness.get_by_label("More").rect();
    for text in ["Read-only", "ETABS not connected"] {
        assert!(present(&harness, text), "{text} keeps an accessible name");
    }
    // Nothing in the row overlaps and nothing runs off the window.
    let mut row: Vec<egui::Rect> = vec![
        harness
            .get_by_role_and_label(Role::ComboBox, "Version")
            .rect(),
        harness.get_by_label("Model").rect(),
        harness.get_by_label("Results").rect(),
        more,
    ];
    row.extend(
        harness
            .query_all_by_label("Read-only")
            .chain(harness.query_all_by_label("ETABS not connected"))
            .map(|node| node.rect()),
    );
    row.sort_by(|a, b| a.left().total_cmp(&b.left()));
    for pair in row.windows(2) {
        assert!(pair[0].right() <= pair[1].left() + 0.5, "{pair:?} overlap");
    }
    assert!(row.last().expect("items").right() <= 900.0);
    // The canvas keeps a usable width between the docked panes.
    set_mode(&mut harness, "Docked panels");
    let canvas = harness.get_by_label("3D model view").rect();
    assert!(canvas.width() >= 380.0, "{canvas:?}");

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num3);
    harness.run();
    assert_eq!(
        selected_tabs(&harness),
        ["Compare"],
        "the active one stays visible"
    );
    harness.get_by_label("More").click();
    harness.run();
    harness.get_by_label_contains("History    Ctrl+4").click();
    harness.run();
    assert_eq!(selected_tabs(&harness), ["History"]);
}

#[test]
fn results_selector_labels_come_before_their_controls() {
    let mut harness = harness(LoadMode::Inline);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    harness.run();
    for name in ["Metric", "Case"] {
        let label = harness
            .get_all_by_role(Role::Label)
            .find(|n| n.accesskit_node().value().as_deref() == Some(name))
            .unwrap_or_else(|| panic!("{name} label"))
            .rect();
        let control = harness.get_by_role_and_label(Role::ComboBox, name).rect();
        assert!(
            label.right() <= control.left(),
            "{name}: {label:?} {control:?}"
        );
        assert!(
            (label.center().y - control.center().y).abs() < 2.0,
            "{name}"
        );
    }
    assert!(present(
        &harness,
        "Story drift · wind · X · no case · all stories · main/v2"
    ));
}

#[test]
fn shrinking_a_wide_window_to_900_points_narrows_the_docked_panes() {
    let mut harness = harness(LoadMode::Inline);
    set_mode(&mut harness, "Docked panels");
    let wide = harness.get_by_label("3D model view").rect().width();
    harness.set_size(egui::vec2(900.0, 700.0));
    settle(&mut harness);
    let canvas = harness.get_by_label("3D model view").rect();
    assert!(
        canvas.width() >= 380.0,
        "panes sized for {wide} pt of canvas kept {canvas:?}"
    );
}

#[test]
fn choosing_another_story_drops_the_member_picked_on_the_old_one() {
    let mut harness = harness(LoadMode::Inline);
    select(&mut harness, "|C0_0|L12");
    assert!(!present(&harness, "Nothing selected"));
    // L1 is the last story row: scroll it into view, then click it.
    row(&harness, "L1  ·  ").scroll_to_me();
    settle(&mut harness);
    row(&harness, "L1  ·  ").click();
    settle(&mut harness);
    let state = harness.state().state();
    let scope = state.session.scope().expect("ready");
    assert_eq!(state.model.story.as_deref(), Some("L1"));
    assert_eq!(state.model.selected(scope), [] as [usize; 0]);
    assert!(present(&harness, "Nothing selected"), "inspector cleared");
    let expected: usize = {
        let document = state.session.ready().expect("ready");
        document
            .mesh()
            .entities()
            .iter()
            .filter(|e| e.story == "L1")
            .map(|e| (e.indices.end - e.indices.start) as usize)
            .sum()
    };
    assert!(expected > 0);
    // The frame the shell hands the view now asks for the L1 story, not the
    // old L12 member.
    let frame = crate::view::ViewFrame {
        document: state.session.ready().expect("ready"),
        scope,
        selected: state.model.selected(scope),
        story: state.model.story.as_deref(),
        picking_allowed: true,
    };
    assert_eq!(
        crate::native_view::highlight_indices(&frame).len(),
        expected
    );
}

/// Extent of a floating card, by its accessible name: the union of what it
/// draws (an egui scope node carries no bounds of its own).
fn card(harness: &Harness<'_, App>, name: &str) -> Option<egui::Rect> {
    let pane = harness.root().children_recursive().find(|node| {
        node.accesskit_node().role() == Role::Pane
            && node.accesskit_node().label().as_deref() == Some(name)
    })?;
    pane.children_recursive()
        .filter(|node| node.accesskit_node().bounding_box().is_some())
        .map(|node| node.rect())
        .reduce(|a, b| a.union(b))
}
/// The widest part of the canvas between the visible cards, above the strip.
fn unobscured(harness: &Harness<'_, App>) -> egui::Rect {
    let canvas = harness.get_by_label("3D model view").rect();
    let strip = harness.get_by_label("Version v1").rect();
    let left = card(harness, "Explorer").map_or_else(|| canvas.left(), |r| r.right());
    let right = card(harness, "Inspector").map_or_else(|| canvas.right(), |r| r.left());
    egui::Rect::from_min_max(
        egui::pos2(left, canvas.top()),
        egui::pos2(right, strip.top() - 14.0),
    )
}

#[test]
fn at_900_points_one_contextual_panel_leaves_the_model_unobscured() {
    let mut harness = harness_sized(LoadMode::Inline, egui::vec2(900.0, 700.0));
    // Default: Explorer only.
    assert!(card(&harness, "Explorer").is_some());
    assert!(card(&harness, "Inspector").is_none());
    let free = unobscured(&harness);
    assert!(free.width() >= 560.0 && free.height() >= 450.0, "{free:?}");
    assert!(
        present(&harness, "Show inspector (I)"),
        "the hidden panel is one click away"
    );

    // A member pick: the Inspector replaces the Explorer, with its tier chip.
    select(&mut harness, "|F|L12");
    assert!(card(&harness, "Explorer").is_none());
    assert!(card(&harness, "Inspector").is_some());
    assert!(present(&harness, "flat at declared face"));
    assert!(present(&harness, "Switch to explorer (I)"));
    let free = unobscured(&harness);
    assert!(free.width() >= 560.0 && free.height() >= 450.0, "{free:?}");

    // Keyboard swaps both ways and keeps the pick.
    let picked = selected(&harness);
    harness.key_press(egui::Key::I);
    settle(&mut harness);
    assert!(card(&harness, "Explorer").is_some() && card(&harness, "Inspector").is_none());
    harness.key_press(egui::Key::E);
    settle(&mut harness);
    assert!(card(&harness, "Inspector").is_some() && card(&harness, "Explorer").is_none());
    assert_eq!(selected(&harness), picked);

    // 900 -> 1280 -> 900: both panels return, nothing reloads, the pick stays.
    let generation = harness.state().state().session.scope().expect("ready");
    harness.set_size(egui::vec2(1280.0, 800.0));
    settle(&mut harness);
    assert!(card(&harness, "Explorer").is_some() && card(&harness, "Inspector").is_some());
    harness.set_size(egui::vec2(900.0, 700.0));
    settle(&mut harness);
    assert!(card(&harness, "Inspector").is_some() && card(&harness, "Explorer").is_none());
    assert_eq!(harness.state().state().session.scope(), Some(generation));
    assert_eq!(selected(&harness), picked);

    // Docked at 900 shows the same single panel beside a wide canvas.
    set_mode(&mut harness, "Docked panels");
    let canvas = harness.get_by_label("3D model view").rect();
    assert!(canvas.width() >= 560.0, "{canvas:?}");
    assert!(present(&harness, "Switch to explorer (I)"));
    assert!(
        !present(&harness, "ALL ENTITIES"),
        "the Explorer is not drawn"
    );
}

#[test]
fn the_explorer_says_when_the_selection_is_outside_its_story_filter() {
    let mut harness = harness(LoadMode::Inline);
    row(&harness, "L5  ·  ").scroll_to_me();
    settle(&mut harness);
    row(&harness, "L5  ·  ").click();
    settle(&mut harness);
    // Exactly L1 (a `contains` match would also find L10-L12).
    let index = {
        let document = harness.state().state().session.ready().expect("ready");
        document
            .mesh()
            .entities()
            .iter()
            .position(|e| e.key.to_string().ends_with("|C0_0|L1"))
            .expect("C0_0 on L1")
    };
    let scope = harness.state().state().session.scope().expect("ready");
    harness.state_mut().dispatch(Action::Pick {
        scope,
        entity: Some(index),
        toggle: false,
        coincident: 0,
    });
    settle(&mut harness);
    let picked = selected(&harness);
    assert_eq!(picked, [index]);
    assert_eq!(harness.state().state().model.story.as_deref(), Some("L5"));
    assert!(present(
        &harness,
        "Selected C0_0 is on L1, outside the L5 filter."
    ));
    harness.get_by_label("Show in Explorer").click();
    settle(&mut harness);
    assert_eq!(harness.state().state().model.story.as_deref(), Some("L1"));
    assert_eq!(selected(&harness), picked, "the selection is kept");
    assert!(!present(&harness, "outside the"));
    assert!(present(&harness, "ENTITIES ON L1"));
}

#[test]
fn compact_panels_keep_a_narrow_inspector_at_every_scale_and_height() {
    for scale in [1.0, 1.25, 1.5] {
        for height in [600.0, 700.0] {
            let context = format!("900 x {height} at {scale}");
            let mut harness = harness_scaled(LoadMode::Inline, egui::vec2(900.0, height), scale);
            assert!(card(&harness, "Explorer").is_some(), "{context}");
            assert!(card(&harness, "Inspector").is_none(), "{context}");
            select(&mut harness, "|F|L12");
            assert!(card(&harness, "Explorer").is_none(), "{context}");
            let inspector = card(&harness, "Inspector").expect("inspector");
            let canvas = harness.get_by_label("3D model view").rect();
            let left = canvas.right()
                - crate::pages::model::MARGIN
                - crate::pages::model::COMPACT_INSPECTOR_WIDTH;
            assert!(inspector.left() >= left - 0.5, "{context}: {inspector:?}");
            let free = unobscured(&harness);
            assert!(
                free.width() >= 600.0 && free.height() >= 330.0,
                "{context}: {free:?}"
            );
            // Readable, not elided: the native-display sentence and the
            // source key appear whole in the narrow card.
            assert!(present(&harness, "flat at declared face"), "{context}");
            assert!(present(&harness, "inst|sh|F|L12"), "{context}");
            // E/I, compact -> wide -> compact, docked: one narrow panel.
            harness.key_press(egui::Key::I);
            settle(&mut harness);
            assert!(card(&harness, "Explorer").is_some(), "{context}: I");
            harness.key_press(egui::Key::E);
            settle(&mut harness);
            assert!(card(&harness, "Inspector").is_some(), "{context}: E");
            harness.set_size(egui::vec2(1280.0, 800.0));
            settle(&mut harness);
            assert!(card(&harness, "Explorer").is_some(), "{context}: wide");
            harness.set_size(egui::vec2(900.0, height));
            settle(&mut harness);
            assert!(
                card(&harness, "Explorer").is_none(),
                "{context}: compact again"
            );
            set_mode(&mut harness, "Docked panels");
            let docked = harness.get_by_label("3D model view").rect();
            assert!(
                docked.width() >= 900.0 - crate::pages::model::COMPACT_INSPECTOR_WIDTH - 2.0,
                "{context}: docked {docked:?}"
            );
        }
    }
}

fn results_demo() -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_max_steps(8)
        .with_size(egui::vec2(1280.0, 800.0))
        .with_context_setup(crate::theme::install)
        .build_eframe(move |cc| {
            App::with_parts(
                Project::public_results_fixture(),
                Box::new(NativeModelView::new(None)),
                Loader::new(LoadMode::Inline, Some(cc.egui_ctx.clone())),
            )
        });
    settle(&mut harness);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    harness
}

#[test]
fn results_chart_and_story_table_show_stored_values_and_link_selection() {
    let mut harness = results_demo();
    assert!(
        present(&harness, "Story drift · wind X · G · W"),
        "chart title"
    );
    assert!(present(&harness, "Cached · freshness not verified"));
    assert!(present(&harness, "Authored fixture"));
    // Table rows carry the stored values, signs kept, in stored order.
    let roof = "Roof  ·  0.00200  ·  -0.00250";
    assert!(present(&harness, roof), "Roof row");
    assert!(present(&harness, "L1  ·  0.00100  ·  -0.00150"), "L1 row");
    assert!(present(&harness, "Stored allowable 0.01000"));
    harness.get_by_label(roof).click();
    settle(&mut harness);
    assert_eq!(
        harness.state().state().results.story.as_deref(),
        Some("Roof")
    );
    assert_eq!(
        harness.get_by_label(roof).accesskit_node().toggled(),
        Some(Toggled::True),
        "the row's state is in the accessibility tree"
    );
    harness.get_by_label(roof).click();
    settle(&mut harness);
    assert_eq!(
        harness.state().state().results.story,
        None,
        "a second click clears"
    );
    // Only the stored responses are offered.
    harness
        .get_by_role_and_label(Role::ComboBox, "Metric")
        .click();
    settle(&mut harness);
    assert!(present(&harness, "Story shear"));
    assert!(!present(&harness, "Story drift · seismic"), "not stored");
    assert!(
        !present(&harness, "Story displacement · wind"),
        "not stored"
    );
    harness.get_by_label("Story moment · stored Mx, My").click();
    settle(&mut harness);
    assert!(present(&harness, "Mx (stored)") && present(&harness, "My (stored)"));
    assert!(
        !present(&harness, "Overturning"),
        "no axis convention is claimed"
    );
    assert!(!present(&harness, "Group"), "profiles have no group");
}

#[test]
fn the_tower_still_says_it_has_no_cached_results() {
    let mut harness = harness(LoadMode::Inline);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    assert!(present(&harness, "No cached results for main/v2"));
    assert!(present(
        &harness,
        "This synthetic snapshot has no cached results."
    ));
    assert!(
        !present(&harness, "Story table"),
        "no table without results"
    );
    assert!(!present(&harness, "Cached · freshness not verified"));
}

#[test]
fn results_fit_a_900_point_window() {
    let mut harness = Harness::builder()
        .with_max_steps(8)
        .with_size(egui::vec2(900.0, 700.0))
        .with_context_setup(crate::theme::install)
        .build_eframe(move |cc| {
            App::with_parts(
                Project::public_results_fixture(),
                Box::new(NativeModelView::new(None)),
                Loader::new(LoadMode::Inline, Some(cc.egui_ctx.clone())),
            )
        });
    settle(&mut harness);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    for node in harness.root().children_recursive() {
        if node.accesskit_node().bounding_box().is_none() {
            continue;
        }
        let rect = node.rect();
        let name = node
            .accesskit_node()
            .label()
            .or_else(|| node.accesskit_node().value())
            .unwrap_or_default();
        assert!(
            rect.right() <= 900.5,
            "{:?} {name:?} at {rect:?} runs off a 900 pt window",
            node.accesskit_node().role()
        );
    }
}

/// Stands in for the Windows chart window: records where the app places it
/// and what it shows, and plays queued page messages through the real bridge.
#[derive(Default)]
struct ChartProbe {
    available: bool,
    synced: usize,
    area: Option<egui::Rect>,
    inbox: Vec<String>,
    bridge: crate::chart_bridge::Bridge,
}
type Probe = std::rc::Rc<std::cell::RefCell<ChartProbe>>;
struct FakeChart(Probe);
impl crate::chart_bridge::EmbeddedChart for FakeChart {
    fn available(&self) -> bool {
        self.0.borrow().available
    }
    fn sync(
        &mut self,
        _ctx: &egui::Context,
        area: Option<egui::Rect>,
        content: Option<crate::chart_bridge::Content>,
    ) -> Vec<Action> {
        let mut probe = self.0.borrow_mut();
        let inbox = std::mem::take(&mut probe.inbox);
        probe.bridge.update(content).expect("update");
        let actions = inbox
            .iter()
            .filter_map(|raw| probe.bridge.accept(raw).ok())
            .collect();
        probe.area = area;
        probe.synced += 1;
        actions
    }
}

fn results_with_chart(available: bool) -> (Harness<'static, App>, Probe) {
    let probe = std::rc::Rc::new(std::cell::RefCell::new(ChartProbe {
        available,
        ..ChartProbe::default()
    }));
    let chart = std::rc::Rc::clone(&probe);
    let mut harness = Harness::builder()
        .with_max_steps(8)
        .with_size(egui::vec2(1280.0, 800.0))
        .with_context_setup(crate::theme::install)
        .build_eframe(move |cc| {
            App::with_parts(
                Project::public_results_fixture(),
                Box::new(NativeModelView::new(None)),
                Loader::new(LoadMode::Inline, Some(cc.egui_ctx.clone())),
            )
            .with_chart(Box::new(FakeChart(chart)))
        });
    settle(&mut harness);
    (harness, probe)
}

/// A page message about the frame the chart shows now.
fn chart_message(probe: &ChartProbe, kind: &str, story: Option<&str>) -> String {
    let frame = probe.bridge.frame().expect("a frame is charted");
    serde_json::json!({
        "protocol": 1, "type": kind, "version": frame.version,
        "source": frame.source, "generation": frame.generation, "story": story,
    })
    .to_string()
}

#[test]
fn the_embedded_chart_covers_the_plot_area_only_on_results_and_never_over_a_popup() {
    let (mut harness, probe) = results_with_chart(true);
    assert!(probe.borrow().synced > 0);
    assert_eq!(probe.borrow().area, None, "hidden on Model");
    assert!(probe.borrow().bridge.frame().is_none());

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    let area = probe.borrow().area.expect("placed on Results");
    let table = harness.get_by_label("STORY TABLE").rect();
    assert!(
        area.width() > 300.0 && area.height() > 200.0,
        "the plot area: {area:?}"
    );
    assert!(area.right() < table.left(), "beside the table, not over it");
    let title = harness.get_by_label_contains("Chart: Story drift").rect();
    assert!(
        title.intersects(area),
        "the reserved area keeps its accessible name"
    );
    {
        let probe = probe.borrow();
        let frame = probe.bridge.frame().expect("charted");
        assert_eq!(frame.plot.title, "Story drift · wind X · G · W");
        let document = harness.state().state().session.ready().expect("ready");
        assert_eq!(frame.version, document.version().to_string());
        assert_eq!(
            Some(frame.source.as_str()),
            harness
                .state()
                .state()
                .cached()
                .map(|s| s.fingerprint.as_str())
        );
        assert_eq!(frame.style.series.len(), frame.plot.series.len());
    }

    // An open selector list would be drawn under the child window: hide it.
    harness
        .get_by_role_and_label(Role::ComboBox, "Metric")
        .click();
    settle(&mut harness);
    assert_eq!(probe.borrow().area, None, "hidden while a list is open");
    assert!(
        probe.borrow().bridge.frame().is_some(),
        "the content stays meanwhile"
    );
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    assert!(probe.borrow().area.is_some(), "back once the list closes");

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    settle(&mut harness);
    assert_eq!(probe.borrow().area, None, "hidden over the Model viewport");
    assert!(probe.borrow().bridge.frame().is_none());
}

#[test]
fn chart_messages_select_only_in_the_frame_shown_and_forward_shortcuts() {
    let (mut harness, probe) = results_with_chart(true);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    let roof = chart_message(&probe.borrow(), "select_story", Some("Roof"));
    probe.borrow_mut().inbox.push(roof.clone());
    harness.run_ok();
    assert_eq!(
        harness.state().state().results.story.as_deref(),
        Some("Roof")
    );
    assert_eq!(
        probe
            .borrow()
            .bridge
            .frame()
            .and_then(|f| f.selected_story.as_deref()),
        Some("Roof"),
        "the selection goes back to the chart in the same generation"
    );
    // Another metric: a new frame. A late click on the old chart is refused.
    harness
        .get_by_role_and_label(Role::ComboBox, "Metric")
        .click();
    settle(&mut harness);
    harness.get_by_label("Story shear").click();
    settle(&mut harness);
    let clear = chart_message(&probe.borrow(), "clear_selection", None);
    probe.borrow_mut().inbox.push(clear);
    harness.run_ok();
    assert_eq!(harness.state().state().results.story, None);
    probe.borrow_mut().inbox.push(roof);
    harness.run_ok();
    assert_eq!(
        harness.state().state().results.story,
        None,
        "stale click refused"
    );
    assert_eq!(probe.borrow().bridge.refused(), 1);
    // Workspace shortcuts typed while the chart has focus still work.
    let mut key: serde_json::Value =
        serde_json::from_str(&chart_message(&probe.borrow(), "shortcut", None)).expect("message");
    key["key"] = "Digit1".into();
    probe.borrow_mut().inbox.push(key.to_string());
    harness.run_ok();
    assert_eq!(harness.state().state().workspace, Workspace::Model);
}

#[test]
fn an_unavailable_chart_is_explicit_without_a_native_chart_fallback() {
    let (mut harness, probe) = results_with_chart(false);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    assert_eq!(probe.borrow().area, None);
    assert!(probe.borrow().bridge.frame().is_none());
    assert!(present(&harness, "ECharts / WebView2 unavailable"));
    let chart = harness.get_by_label_contains("Chart: Story drift").rect();
    click_at(&mut harness, chart.center(), egui::Modifiers::NONE);
    assert_eq!(
        harness.state().state().results.story,
        None,
        "No silent alternate renderer"
    );
    harness
        .get_by_label("Roof  ·  0.00200  ·  -0.00250")
        .click();
    settle(&mut harness);
    assert_eq!(
        harness.state().state().results.story.as_deref(),
        Some("Roof"),
        "Stored table remains readable"
    );
}

#[test]
fn native_navigation_refuses_a_queued_chart_click_in_the_same_frame() {
    let (mut harness, probe) = results_with_chart(true);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut harness);
    let old = chart_message(&probe.borrow(), "select_story", Some("Roof"));
    probe.borrow_mut().inbox.push(old);
    // The shortcut helper runs a modifier-only frame before the key press.
    // Batch the key and modifiers so navigation and IPC share the same frame.
    let input = harness.input_mut();
    input.events.extend([
        egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
        egui::Event::Key {
            key: egui::Key::Num1,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        },
    ]);
    harness.step();
    settle(&mut harness);
    assert_eq!(harness.state().state().workspace, Workspace::Model);
    assert_eq!(harness.state().state().results.story, None);
    assert_eq!(probe.borrow().area, None);
    assert!(probe.borrow().bridge.frame().is_none());
    assert!(probe.borrow().bridge.refused() > 0);
}
