# APP-01B: the native viewport in the TALOS Model workspace (2026-10-28)

**Status: draft, experimental `egui_test` only.** This is a stacked PR on top of
the unmerged APP-01A head (`0994ab7`, #50). It does not depend on the #52 design
branch. The public tower fixture was brought in as its own isolated commit.
EtabExtension #467 stays HOLD. Tracks
[#49](https://github.com/tadoEng/egui_test/issues/49).

| Item | Value |
|---|---|
| Base | `feat/app-01a-native-shell` @ `0994ab7` (#50, unmerged) |
| Backend pin | EtabExtension `9eb1c8792a1d41f3f81c60428afe89f1655782ba` (unchanged) |
| New crate | `crates/lab-viewport`: the shared renderer core, moved out of `examples/support/native_viewport.rs` |
| `Cargo.lock` | one new package entry, `lab-viewport`; the root package and `talos-native` list it, and `talos-native` also lists `web-time`. No new external crate, no version change |
| Toolchain | Rust 1.99.1, egui/eframe/wgpu 0.37 as published, no `[patch]` |

## One renderer, one selection

```text
examples/support/native_viewport.rs ─┐                 ┌─ talos-native NativeModelView
  (own Picking, story overlay,        │  lab-viewport   │   (camera + cached highlight only)
   inspector text, synthetic mesh)    ├─ OrbitCamera ───┤
                                      ├─ RendererCore ──┤   ViewFrame{document, scope,
                                      ├─ ViewportCallback   selected, story} ──► draw
                                      └─ navigate()     │   click ──► ViewIntent::Pick{scope,
                                                        │            entity, toggle, coincident}
                                                        └──► State::apply (refuses stale scope)
```

- **No second renderer.** The shader, orbit camera, GPU mesh residency, highlight
  overlay, paint callback and input mapping moved, unchanged in behaviour, into
  `lab-viewport`. The examples and TALOS both use it. Geometry comes in through a
  `GpuGeometry` trait, so TALOS's `Document` hands the renderer its own
  `lab-e2k-mesh` buffers without copying them.
- **No second selection authority in TALOS.**
  - `NativeModelView` keeps only presentation state (camera, cached highlight
    indices).
  - A click is CPU ray picking against the document's `PickIndex`, which is built
    on the loader thread together with the mesh. It becomes
    `ViewIntent::Pick{scope, entity, toggle, coincident}`, and `State::apply`
    refuses it in a stale scope.
  - The shell's selection (or story) comes back through `ViewFrame` and is drawn
    as the gold overlay.
  - Explorer picks use the same action. A pick with depth-coincident hits is
    disclosed in the Inspector.
- **No parser duplication.** All geometry comes from `lab-e2k-mesh` over the
  pinned `ext-e2k`.

## GPU lifetime

- `lab_viewport::install` registers the renderer once per wgpu render state. A
  second call reuses it, so switching tabs never recreates pipelines or drops the
  resident mesh.
- A hidden Model tab lays out no view and records no paint callbacks.
- The GPU key is the geometry digest. A version switch uploads the new geometry
  once; byte-identical geometry reuses the resident buffers. The selection still
  expires by load generation, as in APP-01A.
- The camera survives a version switch: same direction, same relative zoom and
  pan, re-centred on the new bounds (`OrbitCamera::rebase`). `F` or the **Fit**
  button refits.
- `talos-native` now requests a 24-bit depth buffer. The examples already did, and
  APP-01A did not need one.

## What the user gets in Model

- The real viewport over the public synthetic tower (`main/v1`, `main/v2`; 672
  source entities).
  - Orbit: drag. Pan: Shift + drag or middle drag. Zoom: wheel.
  - **Fit** (`F`).
  - Click to pick, Ctrl+click to add or remove, click empty space to clear.
- Explorer: **Stories** (click to highlight a story; clicking it again clears),
  then the entity list filtered to that story. Choosing or clearing a story
  clears the member selection and its ambiguity note (review of `0ed117f`). The
  behaviour after that is defined, not incidental: a later pick, on any story,
  is drawn and inspected in place of the story highlight, while the Explorer
  list stays filtered to the chosen story until the story changes.
- The Inspector, unchanged from APP-01A (bounded rows, units, backend fidelity
  vs native display), plus the ambiguity warning.
- The demo project is now the tower:
  - v1 has the L12 slab at MIDDLE, drawn as a body.
  - v2 has it at TOP, shown flat with `FlatAtDeclaredFace` and
    `Approximate [ShellInsertionPointIgnored]`.
  - Its L1–L2 perimeter beams are 3 ft deep instead of 2.5 ft.

## Not in this PR

Renderer beautification (edge lines, grid, gradient, selection outline), the
#52 layout and typography, floating panels, the version timeline, ECharts
(APP-01C), `ext-api` (APP-01D), hover highlighting, and any write or ETABS path.

## Test receipts (agent-reported; Linux container)

**Current receipt: `0e7330b`** (`evidence/app-01b/gate-01b-0e7330b.txt`, the
same `gate.sh.txt`), re-run in full after the story-selection fix:
- every step is green except the known G-ENV-1 whole-workspace Clippy exception
  (`webview_stage_a.rs:393` `eprintln!`);
- workspace tests: **153 passed, 0 failed, 7 ignored** (the two new story
  tests);
- GPU, `talos-native` + `lab-viewport`: **33 passed**;
- GPU examples: **41 passed**.

The earlier commit `0ed117f` only added the `69ee396` receipt (docs). The
`69ee396` receipt does not certify `0e7330b`; the line above does.

Earlier gate on `69ee396` (`evidence/app-01b/gate-01b-69ee396.txt`, script
`gate.sh.txt`; `--locked --offline -j 2`):

- check, fmt, doc tests and WASM: **pass**.
- Workspace tests: **151 passed, 0 failed, 7 ignored**. The 7 ignored are the
  GPU-only tests (6 example probes and 1 `lab-viewport` test).
- GPU steps on Mesa lavapipe (`--include-ignored`):
  - `talos-native` 29 and `lab-viewport` 2;
  - examples `native_e2k` 23 and `native_renderer` 18;
  - all pass, so every previously skipped GPU probe now runs green.
- Whole-workspace strict Clippy returns 101 only from the recorded
  `webview_stage_a` `eprintln!` exception (G-ENV-1). `talos-native`,
  `lab-viewport`, `lab-e2k-mesh`, `lab-fixture`, the root lib, bins and tests, and
  every other example are linted explicitly and are clean.

What the tests cover:

- `talos-native` **31** tests:
  - state tests, including story highlight, a story change clearing the member
    selection, pick ambiguity and per-version geometry keys;
  - headless egui_kittest tests through the real `NativeModelView` without a GPU:
    canvas pick of the L12 slab, Ctrl+click adding a column, empty-space clear,
    story highlight from the Explorer, choosing L1 after picking an L12 member
    (selection cleared, the L1 story highlighted), camera kept across versions,
    `F` refit, and a hidden tab doing no view work. All APP-01A UI tests are kept, including
    Inspector paint bounds at 240/320/520 pt × 1.0/1.25/1.5 × dark/light.
- `lab-viewport` **2** tests:
  - the camera rebase test;
  - an opt-in GPU test: one renderer per resources, uploads only on a new key,
    re-upload on change, highlight upload and clear. It passes on Mesa lavapipe.
- Examples with all features, **including the GPU-only probes**:
  - `native_e2k` 23 and `native_renderer` 18, all passing.
  - These are the same totals as the `main` gate (20+3 and 15+3). The 6 probes
    the gate always skipped ran and passed on lavapipe here.
- The kittest wgpu harness has no depth attachment, so app-level GPU evidence
  comes from the real binary instead (next section).

## Real-binary screenshots (Linux, lavapipe; not Windows evidence)

`evidence/app-01b/capture_talos.py` runs the real `talos-native` binary under
Xvfb with Mesa lavapipe. It sends real X11 clicks through XTest: it clicks the
middle of the canvas, then the Results tab. Captures are at 1.0, 1.25 and 1.5
scale:

- `talos-model-*x.png`: the first frame;
- `talos-model-picked-*x.png`: the click picked beam `BX90_0` on L8, drew the gold
  highlight and filled the Inspector;
- `talos-results-*x.png`: Results stays chart-first with no 3D.

Without a window manager, keyboard focus is not given to the window, so the
script clicks the tab instead of sending Ctrl+2. The light theme was not
captured, because eframe follows a system theme that Xvfb does not provide.

## Owner Windows check (public data only)

```powershell
git fetch origin feat/app-01b-viewport
git switch --detach FETCH_HEAD
$env:CARGO_TARGET_DIR = ".codex-target"
cargo test --locked --offline -p talos-native -p lab-viewport -- --include-ignored
cargo test --locked --offline --example native_e2k --all-features -- --include-ignored
cargo run --release --locked --offline -p talos-native
```

1. Model shows the tower. Orbit, pan, zoom and **Fit** / `F` work, with no
   clipped geometry.
2. Click a beam, a column and the roof slab. The gold highlight and the Inspector
   follow each click. Ctrl+click adds; clicking empty space clears. A column in
   front of a slab edge wins over the slab.
3. Explorer: click `L12 · 56`. That story is highlighted when nothing is selected,
   and the list shows its 56 entities. Pick a member, then click `L1`: the
   member and the Inspector clear, and L1 is highlighted.
4. Switch to `v1` in the header. The view stays where it was, and the selection
   clears. Click the roof slab: `MIDDLE`, `CentredMiddle`. Switch back to `v2`:
   `TOP`, `FlatAtDeclaredFace`.
5. Ctrl+2, then Ctrl+1: the model reappears instantly at the same view.
6. Repeat at 100 / 125 / 150 % and after resizing to the minimum window size.
   Record PASS/FAIL, the Windows build and the GPU.
