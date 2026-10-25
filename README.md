# egui Structural Lab

An isolated native Rust UI experiment for structural-engineering workflows.
The default build is standalone. The optional `e2k` feature reuses only the pure
`ext-e2k` parser at a pinned EtabExtension Git revision; it does not modify that
repository or start ETABS.

## Current spike: native egui workbench

Built with Rust 1.99.1, egui/eframe 0.37.0. The first experiment deliberately uses
`egui::Painter` for a lightweight, orthographic structural preview; it is **not**
a native CAD/FEA renderer and does **not** contain Wry or ECharts yet.

The native application explicitly uses eframe's **wgpu backend** to display the
UI and wireframe. The model itself is projected on the CPU and drawn as 2D
lines; it has no custom GPU 3D pipeline, depth buffer, or hidden-surface removal.

It includes:

- A six-storey **synthetic** demo plus exact 50,000- and 100,000-member grid presets.
- Story selection, visibility filters, and member picking using a screen-space distance test.
- Mouse drag left/right to rotate around the vertical axis, up/down to change
  elevation, and wheel to zoom. Orbit stays centered on the building.
- A member inspector showing stable demo keys and endpoints in metres.
- Unit tests for model identity, selection distance, visibility, and projection.
- Headless UI tests for story selection, member picking, visibility controls,
  two-axis orbit, pitch limits, wheel zoom, reset, and simulated resize/DPI changes.
- Native and web entry points carried forward from the upstream template.

## Try it on Windows

```powershell
git clone https://github.com/tadoEng/egui_test.git
cd egui_test
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy --target wasm32-unknown-unknown
cargo fetch --locked
.\check.ps1
cargo run --locked --release
```

The structural workbench from PR #1 is merged into `main`. Start new experiments
from an up-to-date `main`; there is no need to check out the old PR branch.

```powershell
# Repeat checks without network access after fetching dependencies:
.\check.ps1 -Offline
# Native checks only (skip the WASM target):
.\check.ps1 -Offline -SkipWeb
```

`check.ps1` runs formatting, native check, strict Clippy, tests, doc tests, and a
WASM compile check. It stops on the first failure and works from any current
directory. It does not package the website; `trunk build --locked` is a separate
optional step requiring Trunk.

The toolchain is pinned in `rust-toolchain`; dependency resolution is pinned in
`Cargo.lock`. Keep `--locked` when building or running the experiment. The crate
name remains `eframe_template` for now; renaming/packaging is a separate task.

## Automated UI testing

This project uses the official
[`egui_kittest`](https://github.com/emilk/egui/tree/0.37.0/crates/egui_kittest)
test harness. It runs the real `TemplateApp` in memory, queries widgets by their
accessible labels, and feeds synthetic pointer and wheel events through egui.
It opens no native windows, does not move the desktop mouse, and does not read
or change your saved app settings. Tests use bundled fonts rather than relying
on installed system fonts.

```powershell
# Run only the UI interaction suite:
cargo test --locked --offline --lib ui_tests
# Run all UI and logic tests:
cargo test --locked --offline
```

The existing `check.ps1` includes these tests automatically. Tests simulate UI
sizes and 100%/125%/150% scale factors; they do not verify Windows DPI events,
the GPU's rendered pixels, native window behavior, or WebView integration.
`egui_kittest` also supports optional offscreen GPU/image snapshot tests; this
suite currently checks interaction and application state.

For an interactive widget playground, explore the
[official egui demo](https://www.egui.rs/#demo). To explore this app in a browser,
the existing WASM entry point can be served with `trunk serve` after installing
Trunk. Browser support is compile-checked by `check.ps1`; a browser session is a
separate test from this in-process suite.

## Stress testing

Choose **50,000 members** or **100,000 members** in the explorer, or start with:

```powershell
cargo run --locked --release -- --members 100000
```

These are deterministic multi-storey column/beam grids. The final storey may be
partial to produce the exact requested count. Geometry and stable keys are
cached until the model changes. Views fit the model bounds automatically;
orbit, picking, story isolation, and visibility filters remain available.
Turn on **Continuous redraw** to submit frames while idle. The inspector shows
submitted line segments, one-time build/import time, and previous-frame CPU UI
and viewport-construction times. These exclude GPU execution/presentation.

Run repeatable headless CPU measurements without desktop input:

```powershell
cargo run --locked --release --example stress
```

The example tests 48, 50,000, and 100,000 members during synthetic orbit input,
discards three warm-up frames, and prints CSV medians/p95 over 30 samples. UI
construction and CPU tessellation are measured separately; tessellation includes
cloning the submitted shapes. Triangle counts include UI chrome. This is not a
GPU benchmark, memory benchmark, native-window test, or presentation-FPS result.

## Offline ETABS E2K preview

```powershell
# Supply your local export; the example path is intentionally a placeholder.
cargo run --locked --release --features e2k -- --e2k 'C:\local\model.e2k'
# The same adapter through the headless benchmark:
cargo run --locked --release --features e2k --example stress -- --e2k 'C:\local\model.e2k'
```

With `e2k` enabled, the explorer also provides a local path field and **Load E2K**
button. GUI imports run on a worker thread and preserve the current scene on
failure. Model files and their paths are not embedded in this repo or saved as
app settings. Import reads the export only: no COM, ETABS session, export, model
mutation, calculations, or analysis results.

The adapter validates the length-unit header, then uses `ext-e2k::parse_str` and
`project_scene` on the same read-only bytes. It preserves typed entity
keys and story labels, converts declared length units to metres, and draws frame
centerlines and shell boundary rings. Entity counts are frame/shell instances;
shell boundaries submit multiple line segments. Joint counts are reported but
joints are not drawn. Parser scene notes and warning counts are shown explicitly.
Missing/unrecognized length units and writers outside ETABS 17–23 are rejected.

Measured synthetic CPU results are recorded in
[the stress observations](docs/stress-results-2026-10-25.md).

This is the parser's raw connectivity preview: insertion and joint/end offsets,
shell thickness, opening subtraction, and section geometry are not applied.
Braces follow its planar projection. It is not a geometry-fidelity validation
against the ETABS renderer. Private exports stay outside source control.

## Manual interaction checklist

1. Select a story and switch on "Isolate selected story".
2. Toggle columns and beams; hidden elements must not be pickable.
3. Drag left/right and up/down in the dark viewport to orbit; use the wheel to
   zoom. Reset view restores both rotation angles and zoom.
4. Click a structural line; verify the inspector displays its key, kind, story,
   start and end coordinates.
5. Click empty space to clear the member selection.
6. Restart and confirm the persisted controls remain valid.

Use these steps to explore the viewer manually alongside the automated suite.
Actual desktop focus, GPU output, and OS-level DPI behavior still require a
native run. No performance or native 3D GPU claims follow from this prototype.

## Planned **separate** experiment

Track [issue #2](https://github.com/tadoEng/egui_test/issues/2): investigate
`egui + wry (WebView2 on Windows) + ECharts` by embedding
one local, offline ECharts panel. Verify two-way story selection, resizing,
focus, keyboard input, DPI, overlays/z-order, and cleanup before considering
an application-level architecture change.

Keep this lab separate from EtabExtension Alpha UI and live ETABS automation.
