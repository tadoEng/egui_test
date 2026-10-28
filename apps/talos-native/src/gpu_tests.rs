//! Opt-in GPU tests of the whole application with the real wgpu renderer
//! (egui_kittest's wgpu harness). Ignored by default because they need an
//! adapter; run with `--include-ignored` (on Linux, Mesa lavapipe works).
//! `TALOS_CAPTURE_DIR=<dir>` also writes public screenshots of the synthetic
//! tower for review.
use crate::app::App;
use crate::loader::LoadMode;
use crate::native_view::NativeModelView;
use crate::state::Action;
use egui_kittest::{Harness, kittest::Queryable as _};
use lab_viewport::GpuStats;

const HIGHLIGHT: [u8; 3] = [255, 184, 31];

fn gpu_harness(size: egui::Vec2, scale: f32, theme: egui::Theme) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(scale)
        .with_theme(theme)
        .with_max_steps(8)
        .wgpu()
        .build_eframe(|cc| App::new(cc, LoadMode::Inline));
    harness.run_ok();
    harness
}
fn stats(harness: &Harness<'_, App>) -> GpuStats {
    harness
        .state()
        .view()
        .as_any()
        .downcast_ref::<NativeModelView>()
        .and_then(NativeModelView::gpu_stats)
        .expect("wgpu renderer installed")
}
fn render(harness: &mut Harness<'_, App>) -> image::RgbaImage {
    harness.run_ok();
    harness.render().expect("wgpu render")
}
fn select(harness: &mut Harness<'_, App>, needle: &str) {
    let (index, scope) = {
        let state = harness.state().state();
        let document = state.session.ready().expect("ready");
        let index = document
            .mesh()
            .entities()
            .iter()
            .position(|e| e.key.to_string().contains(needle))
            .expect("entity");
        (index, state.session.scope().expect("ready"))
    };
    harness.state_mut().dispatch(Action::Pick {
        scope,
        entity: Some(index),
        toggle: false,
        coincident: 0,
    });
    harness.run_ok();
}
fn switch(harness: &mut Harness<'_, App>, version: &str) {
    let id = crate::project::VersionId::new("main", version);
    harness.state_mut().dispatch(Action::SwitchVersion(id));
    harness.run_ok();
}
/// Pixels inside the 3D canvas that are neither the canvas background nor
/// near-black, and pixels of the highlight colour.
fn canvas_pixels(harness: &Harness<'_, App>, image: &image::RgbaImage, scale: f32) -> (usize, usize) {
    let rect = harness.get_by_label("3D model view").rect();
    let background = lab_viewport::BACKGROUND;
    let (mut drawn, mut highlighted) = (0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        let point = egui::pos2(x as f32 / scale, y as f32 / scale);
        if !rect.shrink(4.0).contains(point) {
            continue;
        }
        let [r, g, b, _] = pixel.0;
        let near = |a: u8, b: u8| a.abs_diff(b) <= 6;
        if !(near(r, background.r()) && near(g, background.g()) && near(b, background.b())) {
            drawn += 1;
        }
        if near(r, HIGHLIGHT[0]) && near(g, HIGHLIGHT[1]) && near(b, HIGHLIGHT[2]) {
            highlighted += 1;
        }
    }
    (drawn, highlighted)
}
fn capture(image: &image::RgbaImage, name: &str) {
    if let Some(dir) = std::env::var_os("TALOS_CAPTURE_DIR") {
        let path = std::path::Path::new(&dir).join(format!("{name}.png"));
        image.save(&path).expect("save capture");
    }
}

#[test]
#[ignore = "needs a GPU adapter; run with --include-ignored"]
fn renderer_lifetime_follows_geometry_not_tabs_and_draws_the_selection() {
    let mut harness = gpu_harness(egui::vec2(1280.0, 800.0), 1.0, egui::Theme::Dark);
    let first = render(&mut harness);
    let start = stats(&harness);
    assert_eq!(start.uploads, 1, "v2 geometry uploaded once");
    assert!(start.draw_records >= 1);
    let (drawn, highlighted) = canvas_pixels(&harness, &first, 1.0);
    assert!(drawn > 10_000, "the tower is drawn ({drawn} px)");
    assert_eq!(highlighted, 0, "nothing selected yet");

    // A hidden Model tab draws nothing; returning re-uploads nothing.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    render(&mut harness);
    let hidden = stats(&harness);
    assert_eq!(hidden.draw_records, start.draw_records, "no draws while hidden");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    let back = render(&mut harness);
    let shown = stats(&harness);
    assert_eq!(shown.uploads, 1, "tab switch keeps the resident mesh");
    assert!(shown.draw_records > hidden.draw_records);
    capture(&back, "talos-model-tower-v2");

    // The shell's selection reaches the GPU overlay.
    select(&mut harness, "|F|L12");
    let selected = render(&mut harness);
    let (_, highlighted) = canvas_pixels(&harness, &selected, 1.0);
    assert!(highlighted > 1_000, "selected roof slab is highlighted ({highlighted} px)");
    assert!(stats(&harness).selection_uploads >= 1);
    capture(&selected, "talos-model-tower-v2-roof-selected");

    // Each version switch uploads its own geometry once; the highlight expires.
    switch(&mut harness, "v1");
    let v1 = render(&mut harness);
    assert_eq!(stats(&harness).uploads, 2, "v1 geometry uploaded");
    assert_eq!(canvas_pixels(&harness, &v1, 1.0).1, 0, "v2 selection not drawn on v1");
    render(&mut harness);
    assert_eq!(stats(&harness).uploads, 2, "no re-upload while v1 stays");
    switch(&mut harness, "v2");
    render(&mut harness);
    assert_eq!(stats(&harness).uploads, 3, "v2 geometry uploaded again");
}

#[test]
#[ignore = "needs a GPU adapter; run with --include-ignored"]
fn captures_model_and_results_at_three_scales_and_two_themes() {
    for (scale, size) in [
        (1.0, egui::vec2(1920.0, 1080.0)),
        (1.25, egui::vec2(1536.0, 864.0)),
        (1.5, egui::vec2(1280.0, 720.0)),
        (1.0, egui::vec2(900.0, 640.0)),
    ] {
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let mut harness = gpu_harness(size, scale, theme);
            select(&mut harness, "|F|L12");
            let model = render(&mut harness);
            let (drawn, highlighted) = canvas_pixels(&harness, &model, scale);
            assert!(drawn > 5_000, "{scale} {theme:?}: tower drawn");
            assert!(highlighted > 100, "{scale} {theme:?}: selection drawn");
            let tag = format!("{}x{}@{scale}-{theme:?}", size.x, size.y).to_lowercase();
            capture(&model, &format!("talos-model-{tag}"));
            harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
            let results = render(&mut harness);
            capture(&results, &format!("talos-results-{tag}"));
        }
    }
}
