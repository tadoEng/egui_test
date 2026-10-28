# REAL-02 source geometry conversion — first slice

`lab-e2k-mesh` is a pure CPU adapter from bounded caller-owned E2K bytes to immutable display vertex/index arrays. It **does not create GPU resources, display a window, load a user directory, read results or implement selection/async loading**. This begins #38; it does not complete its visible workbench or #24 GPU-scale acceptance.

```rust
let mesh = lab_e2k_mesh::build(e2k_bytes, lab_e2k_mesh::Config::default())?;
// One flat vertex/index stream, source-keyed index ranges, canonical origin,
// source-map/projection skips. No per-member GPU buffer or draw is created.
```

## Authority and approximations

- Only pinned `ext-e2k` at9eb1c8792a1d41f3f81c60428afe89f1655782ba parses engineering connectivity and reconstructs stories/world positions: [project_scene and its limitations](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-e2k/src/scene.rs). The adapter retains typed EntityKey, story names/order/elevations, source map and typed projection skips.
- `LengthUnit` conversion produces metres. Normalize around an f64 world origin before reducing display vertices to f32; source centrelines/elevations/origin remain f64. Normalization is a display transform, not source-model certification.
- Frame bodies carry a per-entity `SectionFidelity`. Only an inline `Concrete Rectangular` section with source `D` and `B` is `SourceRectangle`: depth along CSI default local 2, width along local 3, converted to metres. `Concrete Circle` is `CircleBoundingSquare` (its D×D envelope, labelled approximate). Catalog/library (`FILE`) sections, other shapes, unassigned frames and missing/invalid dimensions are `UnsupportedProxy` at the configured fixed size (default 0.1 m square); they are never presented as the assigned section. Default CSI local axes are assumed (vertical member: local 2 = global +X; otherwise upward in the vertical plane through local 1) because the pinned source does not parse section rotation `ANG`. Declared off-centroid cardinal points and joint offsets are reported (`placement_applied = false`) but not drawn; end length offsets are not drawn. `FrameSource`/`ShellSource` carry section, material, shape, dimensions, cardinal/insertion state and shell thickness for an inspector. Beam/Column/Brace tags come directly from the parser enum, never from orientation. Prism topology reuses `lab-fixture::Geometry::OrientedPrism::control_points` and `PRISM_FACE_RINGS`.
- Shell labels are **source Floor/Panel/Other**, not inferred Slab/Wall semantics. Only explicit `opening:false`, planar convex rings that do not host a declared opening can become zero-thickness surfaces. Unknown opening flags, void areas and concave/nonplanar/self-intersecting rings are withheld with typed per-entity reasons and empty ranges. Since the pinned projection does not subtract host holes or bind a void to its host, a shell that is **coplanar with and overlaps** a declared opening (interior overlap, not a shared edge) is withheld as `HostOpeningSubtractionUnavailable`; an opening ring without a plane conservatively withholds every shell whose bounds touch it. Independent shells elsewhere, and walls crossing a floor void's footprint, stay visible. It never silently fills an opening.
- No groups, pier definitions, engineering calculations, material status or member results are invented. The library performs no filesystem/process/network/log/export operations. Private caller data, keys, digests, counts and meshes must remain local.

## Identity, buffers and bounds

Revision carries a full source SHA256 for source/EntityKey/span context **and a separate canonical geometry-buffer SHA256**. Metadata/comment-only source changes can change provenance while retaining the same buffer identity. Proxy changes affect both geometry and its key; camera/UI state is absent. Same bytes reused across versions may share GPU buffers, but a future selection host must additionally scope messages to actual project/version/generation/result identity; a geometry hash is not selection authority or analysis freshness.

The vertex layout is36 bytes (position/normal/colour), indices areu32 and each source entity retains its index range. Bounds, finite coordinates, supported declared units/version, source keys and winding are checked. Default limits are64MiB input,100k projected source records and256MiB **logical vertex/index lengths**. Projection skip records count conservatively against the input-record budget; this is not a structural-element metric. Counts/indices are checked and fallible buffer reservation is used. These limits do not bound total parser/process memory, allocator capacity or parse latency. There is no worker cancellation or incremental triangulation claim.

## Validation and next connection

Public authored two-story E2K fixtures test typed source keys,3.048m conversion, source story order, known buffer counts, winding/unit normals/outward normals, an inclined projected frame, unknown units/version, source/config/geometry identity, holes/unknown flags/concavity/nonplanarity, bad connectivity, budgets and large-origin precision. No private model or derived measurement is used. `cargo test -p lab-e2k-mesh --locked --offline` and strict crate Clippy are the focused checks; the exact whole-workspace gate must be recorded separately.

The separately reviewed optional [native E2K GPU preview](../../docs/native/e2k-gpu-preview.md) now adapts these arrays into the existing RendererCore/ViewportCallback using an ownership transfer, full geometry digest key and distinct buffer/draw counters. The library remains CPU-only. Async last-good/cancel/stale suppression, #37 checked snapshot/result attachment, version-scoped story selection,100k rendering and presented timings remain pending. No ext-api/CalcOutput dependency or production edit is introduced here; no migration gate is closed.

## Exact-source verification

At codea56d8eaa2767457fb9ad476f10b8089b5511a1e3, the full Windows `check.ps1 -Offline` gate passes85 CPU executions (11 new conversion cases,10 shared repetitions),2 opt-in GPU instances ignored, native all-target/all-feature, fmt/strict Clippy/docs0/WASM. [Public receipt](../../docs/native/evidence/real02-conversion/windows-validation.json) preserves the exact source/log/fixture scope. These remain CPU-only arrays and tests, not GPU allocations, actual100k loading or a visible real-model workbench. Final evidence-only changes preserve compiled inputs.
