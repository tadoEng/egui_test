# Native renderer source map

Assessment date: 2026-10-26. SolidCraft was inspected read-only at commit
[`4f2c133d01d1b3d8bae026af2ae76b5c677949ac`](https://github.com/tangency-labs/solidcraft/tree/4f2c133d01d1b3d8bae026af2ae76b5c677949ac).
The lab base is `61f6b74adbbd38255a5736fb0de7f432a3e82cd4`.

## Compatibility and licence

- The lab resolves egui, eframe and egui-wgpu 0.37.0 with wgpu 30.0.1. SolidCraft declares
  egui/eframe/egui-wgpu 0.36 and wgpu 30, and its lockfile resolves egui-wgpu 0.36.2 and wgpu
  30.0.1. The GPU API major therefore matches, while the egui integration version does not.
- SolidCraft patches egui-wgpu locally. Its changes cover effective MSAA, surface retry/recreate,
  device-loss detection, size clamping and staging-buffer failures; they are not part of stock
  egui-wgpu 0.37. The patch must not be copied or pinned into this lab as part of renderer core.
- SolidCraft and its original renderer sources are MIT OR Apache-2.0. The vendored egui-wgpu is
  also MIT OR Apache-2.0. Any future copied or substantially adapted source must retain the chosen
  licence notices and record attribution. This slice writes a smaller renderer against the public
  stock 0.37 callback API; it does not import SolidCraft crates, shaders, CAD geometry or vendor
  patches.

Manifest evidence: [workspace versions and patch](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/Cargo.toml#L93-L106),
[egui-wgpu patch selection](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/Cargo.toml#L154-L163),
[vendor patch and licence record](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/vendor/README.md#L273-L320).

## Exact source inventory

| Source and symbols | Classification | Native-lab decision |
|---|---|---|
| [`crates/ui-egui/src/gpu.rs`: `install`, `ViewportCallback`, `CallbackTrait::prepare/paint`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/ui-egui/src/gpu.rs#L22-L149) | **Reusable after compatibility** | Use the same stock callback/resource lifecycle, rewritten for egui-wgpu 0.37. Do not use SolidCraft-only `RenderState.msaa_samples` or `device_lost` fields. |
| [`crates/gpu/src/view.rs`: `GpuScene`, block/instance scene data](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/gpu/src/view.rs#L266-L444) | **Learn** | Confirms CPU scene data should be independent of UI state. CAD planned bodies, transparency classes, LOD and block types are unnecessary for the first structural proof. |
| [`crates/gpu/src/view.rs`: `Batches::take`, `instanced`, resident GPU caches](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/gpu/src/view.rs#L939-L1118) | **Adapt** | Keep a geometry revision key and upload vertex/index buffers only when it changes. The first slice needs one mesh cache, not CAD body chunks, tessellation compute or LOD. |
| [`crates/gpu/src/view.rs`: `GpuTarget`, depth helpers](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/gpu/src/view.rs#L1511-L1566) | **Learn** | Depth format, clear value and compare function must agree. Core uses stock eframe's 24-bit depth attachment; reversed float depth is deferred. |
| [`crates/gpu/src/view.rs`: `Renderer::prepare/paint`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/gpu/src/view.rs#L2216-L2368) | **Adapt** | Upload changed geometry in `prepare`, update the camera uniform each frame, and issue indexed draws in `paint`. Supersampling, OIT, image textures and still-frame blits are not needed. |
| [`assets/shaders/viewport.wgsl`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/assets/shaders/viewport.wgsl) | **Not needed** | It implements CAD sections, lines, analysis, images and multiple scene classes. A small original normal-lit shader makes the structural proof easier to review. |
| [`crates/render/src/camera.rs`: `Camera` projection, ray, orbit and fit](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/render/src/camera.rs#L221-L468) | **Adapt** | Use a small orbit/zoom/fit camera with a projection updated from the callback rectangle's aspect ratio. Picking rays are deferred to the picking PR. |
| [`crates/ui-egui/src/navigation.rs`: shared navigation entry point](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/ui-egui/src/navigation.rs#L365-L414) | **Learn** | Input belongs in app/UI glue, not GPU resources. The example owns only orbit, wheel zoom and Fit; CAD tool arbitration and 3D-mouse handling are out of scope. |
| [`crates/ui-egui/src/viewport.rs`: `Hit`, `Proj`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/ui-egui/src/viewport.rs#L181-L276), [`pick_hits`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/ui-egui/src/viewport.rs#L1620-L1890) | **Learn** | SolidCraft picking is CPU ray/BVH picking, filters hidden bodies first, keeps the nearest ray hit for occlusion, and maps CAD-specific hits to selections. It is not GPU ID picking and is excluded from core. |
| [`crates/gpu/src/capture.rs`: `Capturer`, readback, scene conversion](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/gpu/src/capture.rs#L35-L315) | **Adapt** | Use a bounded offscreen texture, depth attachment, padded copy-to-buffer and pixel assertion for a deterministic native GPU test. Do not import CPU fallback or product-render features. |
| [`crates/gpu/tests/depth.rs`: depth/occlusion regression patterns](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/gpu/tests/depth.rs#L70-L132) | **Learn** | Assert pixels from overlapping near/far synthetic faces; do not treat compilation or CPU tessellation as GPU evidence. |
| [`vendor/egui-wgpu/src/lib.rs`: `DeviceLost`, `ConfigureRetry`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/vendor/egui-wgpu/src/lib.rs#L135-L190) and [surface retry](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/vendor/egui-wgpu/src/lib.rs#L784-L922) | **Not needed in core** | Stock egui-wgpu 0.37 has no equivalent public device-loss state. A callback cannot protect egui's own earlier queue uploads. Full device-loss recovery therefore needs a separate host-level compatibility spike or upstream solution. |
| [`vendor/egui-wgpu/src/winit.rs`: acquire/probe/prepare ordering](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/vendor/egui-wgpu/src/winit.rs#L646-L805) | **Learn** | Important reliability ordering, but it changes the host painter and is outside a renderer callback. Record as a later blocker, not an implied core capability. |
| [`apps/solidcraft/src/device_lost.rs`: restart and software fallback](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/apps/solidcraft/src/device_lost.rs#L1-L58) | **Not needed in core** | Process restart, recovery journals and software-adapter policy belong to the desktop-runtime/reliability stage. |

## Boundary conclusion

SolidCraft demonstrates a sound separation: render math and scene descriptions below the UI,
GPU resources in a renderer, and a very small egui callback adapter. Its CAD mesh model,
selection types, tessellation compute, material system and vendor-patched window painter are not
drop-in components for this structural viewer. The minimal safe transfer is the architecture,
not the codebase.

