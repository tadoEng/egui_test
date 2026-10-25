# egui Structural Lab

An isolated native Rust UI experiment for future structural-engineering workflows.
This repository does **not** modify or depend on EtabExtension.

## Current spike: native egui workbench

Built with Rust 1.99.1, egui/eframe 0.37.0. The first experiment deliberately uses
`egui::Painter` for a lightweight, orthographic structural preview; it is **not**
a native CAD/FEA renderer and does **not** contain Wry or ECharts yet.

It includes:

- A six-storey **synthetic** frame, with four columns and four beams per storey.
- Story selection, visibility filters, and member picking using a screen-space distance test.
- Mouse drag to rotate around the vertical axis and wheel to zoom.
- A member inspector showing stable demo keys and endpoints in metres.
- Unit tests for model identity, selection distance, visibility, and projection.
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
optional step requiring Trunk. Desktop interaction checks below remain manual.

The toolchain is pinned in `rust-toolchain`; dependency resolution is pinned in
`Cargo.lock`. Keep `--locked` when building or running the experiment. The crate
name remains `eframe_template` for now; renaming/packaging is a separate task.

## Interaction checklist

1. Select a story and switch on "Isolate selected story".
2. Toggle columns and beams; hidden elements must not be pickable.
3. Drag in the dark viewport to rotate; use the wheel to zoom.
4. Click a structural line; verify the inspector displays its key, kind, story,
   start and end coordinates.
5. Click empty space to clear the member selection.
6. Restart and confirm the persisted controls remain valid.

These checks require a desktop run; unit tests cover the underlying pure logic
only. No performance or native GPU claims follow from this prototype.

## Planned **separate** experiment

Track [issue #2](https://github.com/tadoEng/egui_test/issues/2): investigate
`egui + wry (WebView2 on Windows) + ECharts` by embedding
one local, offline ECharts panel. Verify two-way story selection, resizing,
focus, keyboard input, DPI, overlays/z-order, and cleanup before considering
an application-level architecture change.

**Do not** migrate EtabExtension Alpha UI or add an ETABS integration to this lab.
