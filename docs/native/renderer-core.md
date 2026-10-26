# Native wgpu renderer core

This slice is an independently runnable proof of real GPU 3D rendering hosted by egui. It does
not replace the existing Painter experiment.

Run it on native Windows with:

```powershell
cargo run --release --locked --offline --example native_renderer --features native-renderer
```

The example uses stock eframe/egui-wgpu 0.37 and the already locked wgpu 30.0.1. It creates one
cached native vertex buffer, index buffer, camera uniform, depth-tested render pipeline and egui
paint callback. Orbit and wheel zoom update only the uniform. Fit derives the camera target,
radius and distance from geometry bounds. Eframe owns the native surface and recreates its
24-bit depth attachment when the window size changes; the callback derives projection aspect
from its current rectangle.

## Geometry truth table

| Item | Core status | Fidelity statement |
|---|---|---|
| Beam | Implemented | Synthetic rectangular prism between endpoints, with flat face normals. It is not an authoritative section shape. |
| Column | Implemented | Synthetic rectangular prism between endpoints, with flat face normals. It is not an authoritative section shape. |
| Shell | Implemented | One zero-thickness quadrilateral face with a supplied normal. |
| World transform | Implemented | Geometry is baked in world coordinates; no per-object transform buffer yet. |
| Local axes/orientation | Unsupported for source models | Synthetic prism cross axes are derived deterministically from the member line. No ETABS local-axis metadata is consumed. |
| Story elevation | Synthetic only | Coordinates are authored directly in the deterministic fixture. |
| Real section dimensions | Unsupported | No section catalogue/profile geometry is consumed. |
| Insertion/cardinal offsets | Unsupported | No source metadata is consumed or inferred. |
| End offsets/releases | Unsupported | No source metadata is consumed or inferred. |
| Shell openings | Unsupported | The shell is a single unperforated quad. |

These limitations are also visible in the example. No result from this fixture is evidence of E2K
or ETABS geometry fidelity.

## Verification

```powershell
cargo fmt --check --all
cargo test --locked --offline --example native_renderer --features native-renderer
cargo test --locked --offline --example native_renderer --features native-renderer -- --ignored --nocapture
cargo clippy --locked --offline --example native_renderer --features native-renderer -- -D warnings
cargo check --locked --offline --all-targets --all-features --target wasm32-unknown-unknown
```

The example tests verify deterministic vertex/index topology, finite unit normals, valid indices,
camera limit/resize math and a native offscreen GPU readback where a nearer red face must occlude a
later-drawn blue face. The GPU test also checks that a changed camera/aspect reuses the resident
vertex buffer. It is ignored by default and must be explicitly invoked; absence of a GPU then
fails rather than silently passing. It prints the adapter. It is GPU pixel/depth evidence, not a
native-window capture or Windows interaction test.

The explicit Windows probe passed on **NVIDIA GeForce RTX 3050 Ti Laptop GPU** on 2026-10-26.
The machine has a Ryzen 5 6600H (6 cores/12 logical processors), 16,312,721,408 bytes physical
RAM, Windows 11 build 26300, and NVIDIA driver 32.0.15.9174. The 64×64 RGBA8 offscreen target
uses Depth24Plus, one sample, normal depth (clear 1, compare Less). This was a correctness probe;
no GPU execution time, presentation interval, FPS, or performance result was measured.

`check.ps1 -Offline` passed: formatting, native all-target/all-feature check, strict Clippy,
23 existing library tests plus 2 renderer CPU tests, doc tests and WASM all-target/all-feature
check. The GPU probe is a separate explicit run (1 passed). The Windows release example built
with `--locked --offline` and produced a 15,776,256-byte EXE. Cargo reported 4m 58s for this
partly cached build while other tracks were compiling; it is not a controlled clean build-time
benchmark or a size comparison with another host.

![Synthetic offscreen depth probe: the nearer red face hides the later blue face](depth-probe.png)

To reproduce the optional image, set `NATIVE_GPU_CAPTURE` to a `.png` path before the explicit
GPU test. This writes synthetic test pixels only. The capture is not a screenshot of a native
window or of the structural frame example.

## Acceptance status for this slice

- **PASS (CPU tests and explicit hardware probe):** synthetic topology/normals/index bounds,
  camera math at zoom/pitch/aspect limits, native buffer reuse and depth occlusion.
- **PARTIAL (implemented, Windows interaction NOT TESTED):** callback-hosted structural example,
  orbit/fit/zoom and resize. These are not accepted from compilation alone.
- **PARTIAL:** GPU depth/occlusion is covered by offscreen readback. A visible native window is
  runnable, but Windows capture and manual driver interaction remain separate evidence.
- **BLOCKED in stock callback scope:** robust device-loss recovery. Stock egui-wgpu 0.37 performs
  its own queue uploads before custom callbacks and exposes no SolidCraft-style loss state. The
  vendor patch was intentionally not copied.
- **NOT TESTED / deferred:** stable entity picking, hidden/filter/behind-camera negative cases,
  selection highlighting, story isolation, section/clipping planes, production Three.js
  comparison, GPU timestamps, presented-frame pacing, memory/VRAM and click latency.

