# egui Structural Lab

An isolated native Rust UI experiment for future structural-engineering workflows.
This repository does **not** modify or depend on EtabExtension.

## Current spike: native egui workbench

Built with Rust 1.99.1, egui/eframe 0.37.0. The first experiment deliberately uses
`egui::Painter` for a lightweight, orthographic structural preview; it is **not**
a native CAD/FEA renderer and does **not** contain Wry or ECharts yet.

The native application explicitly uses eframe's **wgpu backend** to display the
UI and wireframe. The model itself is projected on the CPU and drawn as 2D
lines; it has no custom GPU 3D pipeline, depth buffer, or hidden-surface removal.

It includes:

- A six-storey **synthetic** frame, with four columns and four beams per storey.
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

**Do not** migrate EtabExtension Alpha UI or add an ETABS integration to this lab.
