//! In-process tests of the real app UI; no native window or desktop input.

use egui_kittest::{Harness, kittest::Queryable as _};

use super::*;

fn harness() -> Harness<'static, TemplateApp> {
    Harness::builder()
        .with_size(egui::vec2(1200.0, 760.0))
        // Wheel input settles over several frames; keep a bounded animation budget.
        .with_max_steps(32)
        .build_eframe(|cc| TemplateApp::new(cc))
}

fn viewport(harness: &Harness<'_, TemplateApp>) -> egui::Rect {
    harness.get_by_label("Structural viewport").rect()
}

fn click_at(harness: &mut Harness<'_, TemplateApp>, position: egui::Pos2) {
    harness.hover_at(position);
    harness.drag_at(position);
    harness.drop_at(position);
    harness.run();
}

fn drag(harness: &mut Harness<'_, TemplateApp>, delta: egui::Vec2) {
    let start = viewport(harness).center();
    let end = start + delta;
    harness.hover_at(start);
    harness.drag_at(start);
    harness.hover_at(end);
    harness.drop_at(end);
    harness.run();
}

fn wheel(harness: &mut Harness<'_, TemplateApp>, position: egui::Pos2, delta: f32) {
    harness.hover_at(position);
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, delta),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
}

fn column_position(harness: &Harness<'_, TemplateApp>) -> egui::Pos2 {
    // Midpoint of C1@S3, well away from its endpoints.
    let app = harness.state();
    project(
        [-3.0, -2.5, 10.0],
        viewport(harness).center(),
        app.yaw,
        app.pitch,
        app.zoom,
    )
}

#[test]
fn story_selection_updates_inspector_and_clears_member() {
    let mut harness = harness();
    let position = column_position(&harness);
    click_at(&mut harness, position);
    assert!(harness.state().selected_member.is_some());

    harness.get_by_label("Story 2").click();
    harness.run();

    assert_eq!(harness.state().selected_story, 2);
    assert!(harness.state().selected_member.is_none());
    harness.get_by_label("Active story: 2");
}

#[test]
fn visibility_controls_isolate_and_hide_members() {
    let mut harness = harness();
    let count = |harness: &Harness<'_, TemplateApp>| {
        sample_members()
            .iter()
            .filter(|member| harness.state().visible(member))
            .count()
    };

    harness.get_by_label("Isolate selected story").click();
    harness.run();
    assert_eq!(count(&harness), 8);

    harness.get_by_label("Beams").click();
    harness.run();
    assert_eq!(count(&harness), 4);

    harness.get_by_label("Columns").click();
    harness.run();
    assert_eq!(count(&harness), 0);
}

#[test]
fn horizontal_drag_rotates_both_ways_without_tilting() {
    let mut harness = harness();
    drag(&mut harness, egui::vec2(60.0, 0.0));
    assert!(harness.state().yaw > DEFAULT_YAW);

    drag(&mut harness, egui::vec2(-120.0, 0.0));
    assert!(harness.state().yaw < DEFAULT_YAW);
    assert_eq!(harness.state().pitch.to_bits(), DEFAULT_PITCH.to_bits());
}

#[test]
fn vertical_drag_tilts_both_ways_without_changing_yaw() {
    let mut harness = harness();
    drag(&mut harness, egui::vec2(0.0, 40.0));
    assert!(harness.state().pitch > DEFAULT_PITCH);

    drag(&mut harness, egui::vec2(0.0, -80.0));
    assert!(harness.state().pitch < DEFAULT_PITCH);
    assert_eq!(harness.state().yaw.to_bits(), DEFAULT_YAW.to_bits());
}

#[test]
fn pitch_stops_at_both_poles() {
    for direction in [-1.0, 1.0] {
        let mut harness = harness();
        harness.state_mut().pitch = direction * (MAX_PITCH - 0.05);
        harness.run();
        drag(&mut harness, egui::vec2(0.0, direction * 40.0));
        assert_eq!(harness.state().pitch, direction * MAX_PITCH);
    }
}

#[test]
fn wheel_zooms_in_and_out_and_obeys_limits() {
    let mut harness = harness();
    let position = viewport(&harness).center();
    wheel(&mut harness, position, 120.0);
    assert!(harness.state().zoom > 15.0);

    wheel(&mut harness, position, -120.0);
    assert!((harness.state().zoom - 15.0).abs() < 0.01);

    wheel(&mut harness, position, 10_000.0);
    assert_eq!(harness.state().zoom, 40.0);

    wheel(&mut harness, position, -10_000.0);
    assert_eq!(harness.state().zoom, 5.0);
}

#[test]
fn wheel_outside_the_viewport_does_not_zoom() {
    let mut harness = harness();
    let position = harness.get_by_label("Reset view").rect().center();
    wheel(&mut harness, position, 120.0);
    assert_eq!(harness.state().zoom, 15.0);
}

#[test]
fn reset_restores_yaw_pitch_and_zoom() {
    let mut harness = harness();
    drag(&mut harness, egui::vec2(60.0, 40.0));
    let position = viewport(&harness).center();
    wheel(&mut harness, position, 120.0);

    harness.get_by_label("Reset view").click();
    harness.run();

    let app = harness.state();
    assert_eq!(
        (app.yaw, app.pitch, app.zoom),
        (DEFAULT_YAW, DEFAULT_PITCH, 15.0)
    );
}

#[test]
fn clicking_a_member_updates_the_inspector() {
    let mut harness = harness();
    let position = column_position(&harness);
    click_at(&mut harness, position);

    assert_eq!(harness.state().selected_member.as_deref(), Some("C1@S3"));
    harness.get_by_label("C1@S3");
    harness.get_by_label("Kind: Column");
}

#[test]
fn clicking_empty_space_clears_member_selection() {
    let mut harness = harness();
    let position = column_position(&harness);
    click_at(&mut harness, position);
    assert!(harness.state().selected_member.is_some());

    let empty = viewport(&harness).left_top() + egui::vec2(30.0, 60.0);
    click_at(&mut harness, empty);
    assert!(harness.state().selected_member.is_none());
    harness.get_by_label("Click a member in the viewport.");
}

#[test]
fn hidden_members_cannot_be_picked() {
    let mut harness = harness();
    let position = column_position(&harness);
    click_at(&mut harness, position);
    assert!(harness.state().selected_member.is_some());

    harness.get_by_label("Columns").click();
    harness.get_by_label("Beams").click();
    harness.run();
    click_at(&mut harness, position);
    assert!(harness.state().selected_member.is_none());
}

#[test]
fn viewport_remains_usable_after_resize_and_dpi_changes() {
    let mut harness = harness();
    for (size, scale) in [
        (egui::vec2(1200.0, 760.0), 1.0),
        (egui::vec2(780.0, 520.0), 1.25),
        (egui::vec2(780.0, 520.0), 1.5),
    ] {
        harness.set_size(size);
        harness.set_pixels_per_point(scale);
        harness.run();
        let rect = viewport(&harness);
        assert!(harness.ctx.content_rect().contains_rect(rect));
        assert!(rect.width() >= 100.0 && rect.height() >= 200.0);

        let position = column_position(&harness);
        click_at(&mut harness, position);
        assert_eq!(harness.state().selected_member.as_deref(), Some("C1@S3"));
    }
}
