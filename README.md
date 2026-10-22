# egui Structural Lab

An isolated native Rust UI experiment for future structural-engineering workflows.
This repository does **not** modify or depend on EtabExtension.

## Current spike: native egui workbench

Built with Rust 1.95, egui/eframe 0.36.0. The first experiment deliberately uses
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
rustup toolchain install 1.95.0
cargo run --release
```

For the experimental PR branch, check out
`feat/native-structural-workbench-spike` before running these commands.

```powershell
cargo test --lib
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

The dependency versions and `rust-toolchain` are pinned in the repo. The crate
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

Investigate `egui + wgpu + wry (WebView2 on Windows) + ECharts` by embedding
one local, offline ECharts panel. Verify two-way story selection, resizing,
focus, keyboard input, DPI, overlays/z-order, and cleanup before considering
an application-level architecture change.

**Do not** migrate EtabExtension Alpha UI or add an ETABS integration to this lab.
