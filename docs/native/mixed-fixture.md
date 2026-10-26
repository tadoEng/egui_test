# WB-01: authored mixed structural fixture

Scope: [#23](https://github.com/tadoEng/egui_test/issues/23), under [#22](https://github.com/tadoEng/egui_test/issues/22). Starts from renderer-core merge `edab698616d4bee13971c50dcdb0811e26b3a458`. Assessment **PARTIAL** until exact-head tests and independent review. No rendering, loading, picking, visibility UI, Windows visual evidence or engineering-source fidelity is claimed by this data-only slice.

`crates/lab-fixture` is a std-only workspace crate; the application does not depend on it yet. No API/parser, window/GPU, random, I/O or serialization dependency. Existing Painter 48/50,000/100,000 fixtures and the standalone renderer example are unchanged. `check.ps1` already checks/tests all workspace members, including WASM compilation.

## Contract

All coordinates are f64 metres, right-handed world axes, Z up. Beam/Column/Wall/Slab are explicit authored kinds. Frames are axis-aligned rectangular **solid test prisms**, walls are vertical **zero-thickness shell surfaces**, and slabs are planar convex shell surfaces (including one sloped surface). No engineering section, material, local-axis, insertion-point, offset, shell thickness, ETABS group or source-provenance semantics are implemented. Shells use authored outward normals; a zero-thickness surface has one authored front side, not the outside of a volume.

Stable IDs have the form `LAB/Column/S01/00000`. They are unique within a preset. Selection must use **(GeometryRevision, LabId)**; the same ID in a different preset is not the same selection. A validated `Fixture` exposes shared references only. Its revision is `(preset, formula_version=1)`, an authored formula identity, **not** an E2K fingerprint, hash or authoritative `EntityKey`. Any geometry, key or story-attribution change must bump the formula version. Future metadata-only revisions may be separate from the geometry cache identity; this slice has no metadata editing API.

Stories are one-based with ascending elevations, 3 m apart. Columns/walls span the previous level to the named **upper** level; beams/slabs belong to their authored floor. The sloped slab belongs to story 2 despite extending above that elevation. Multi-story members, split-floor attribution, basements and other source conventions are unsupported.

Groups overlap intentionally. `Core` contains four core walls per large floor (one per truth-table floor); `Perimeter` contains all boundary columns and beams; `Lateral System` contains the perimeter and **all** walls. Zone A contains elements whose mean control-point X is at most mid-width + 2 m; Zone B contains elements whose mean X is at least mid-width - 2 m. Thus the middle strip is in both; a tiny fixture may be entirely in both zones. `Empty` has zero members. These are authored lab rules, not production grouping logic. Sorted exact-ID indexes are provided for groups and stories.

## Manifest

No random seed: the formula is explicit. Large grids have 69 stories, 4 m bays, `(nx+1)(ny+1)` columns, `nx(ny+1)+ny(nx+1)` beams, six walls and `nx*ny` slabs **per story**. Four walls form a rectangular core at quarter/three-quarter width/depth; two separate shear walls sit at one-eighth/seven-eighth width. Prism widths are 0.4 m for columns, 0.3 m for beams. Slabs have no thickness or holes.

| Preset | Bays X×Y | Stories | Beam | Column | Wall | Slab | Structural entities |
|---|---:|---:|---:|---:|---:|---:|---:|
| TruthTable | authored | 2 | 8 | 8 | 4 | 3 | 23 |
| Thousand | 1×1 | 69 | 276 | 276 | 414 | 69 | 1,035 |
| TenThousand | 5×6 | 69 | 4,899 | 2,898 | 414 | 2,070 | 10,281 |
| FiftyThousand | 13×13 | 69 | 25,116 | 13,524 | 414 | 11,661 | 50,715 |
| HundredThousand | 18×19 | 69 | 49,749 | 26,220 | 414 | 23,598 | 99,981 |

Per-floor entity totals for large presets are respectively 15, 149, 735, 1,449. TruthTable has 11 on story 1 and 12 on story 2.

| Preset | Core | Perimeter | Zone A | Zone B | Lateral System | Empty |
|---|---:|---:|---:|---:|---:|---:|
| TruthTable | 2 | 16 | 16 | 17 | 20 | 0 |
| Thousand | 276 | 552 | 1,035 | 1,035 | 966 | 0 |
| TenThousand | 276 | 3,036 | 6,555 | 6,555 | 3,450 | 0 |
| FiftyThousand | 276 | 7,176 | 28,221 | 28,221 | 7,590 | 0 |
| HundredThousand | 276 | 10,212 | 54,096 | 54,096 | 10,626 | 0 |

Group counts do **not** sum to the entity count. For large presets each row's group count is evenly distributed across the 69 stories.

| Preset | Geometry control points | Reference mesh vertices | Reference triangles | Actual GPU batches/draw calls |
|---|---:|---:|---:|---|
| TruthTable | 157 | 413 | 207 | N/A: no renderer |
| Thousand | 6,348 | 15,180 | 7,590 | N/A |
| TenThousand | 72,312 | 197,064 | 98,532 | N/A |
| FiftyThousand | 357,420 | 975,660 | 487,830 | N/A |
| HundredThousand | 703,800 | 1,919,304 | 959,652 | N/A |

Control points count eight corners/prism and each shell boundary. Reference mesh counts assume 24 flat-shaded face vertices and 12 triangles/prism; a shell with n boundary vertices has n vertices and n−2 fan triangles. `PRISM_FACE_RINGS` defines the six outward CCW quads; each quad splits (0,1,2)/(0,2,3). These are topology counts, **not allocated GPU buffers or measured performance**. Four kind partitions could be batched by the future renderer, but no batch count is predicted here.

## Small truth table and refusal behavior

At elevations 3/6 m: four perimeter columns at (0,0), (6,0), (0,4), (6,4); four perimeter beams; a core wall from (2,2) to (4,2); a separate shear wall from (5,1) to (5,3); one rectangular slab. Story 2 adds the convex pentagon from `(8+x,y,6+0.2x+0.1y)` for `(x,y) = (0,0),(3,0),(4,2),(2,4),(0,3)`. Its unit normal is proportional to `(-0.2,-0.1,1)`. Whole-fixture bounds are `(-0.2,-0.2,0)` to `(12,4.2,7)` m. Large bounds are `(-0.2,-0.2,0)` to `(4nx+0.2,4ny+0.2,207.15)` m, including beam/column extents.

The occlusion oracle uses ray origin `(0,-10,1.5)`, unit direction +Y: near `LAB/Column/S01/00000` at `(0,-0.2,1.5)`, far `LAB/Column/S01/00002` at `(0,3.8,1.5)`. No other element intersects this ray earlier. This is a known geometry oracle for WB-03, not proof of an implemented picker or camera-to-pixel mapping.

Openings are **unsupported**: any nonempty opening list produces `UnsupportedOpening`, with no fixture returned. Validation also rejects nonfinite/out-of-range (>1,000 km magnitude) coordinates, invalid/duplicate stories or IDs, duplicate group references or an assigned `Empty` group, zero-size prisms, kind/shape mismatch, degenerate shell edges/fans, wrong/non-unit normals, nonplanarity, concavity and self-intersection. Typed enums exclude unknown group/kind values. No fallback silently patches bad geometry. Raw malformed input is internal to the test crate; consumers obtain validated immutable fixtures through `Fixture::generate` only.

## Verification

Planned exact-head commands (Windows evidence pending while the separate controlled API Release build runs):

```powershell
cargo test -p lab-fixture --locked --offline
cargo clippy -p lab-fixture --all-targets --locked --offline -- -D warnings
./check.ps1 -Offline
```

Regression cases compare two complete generations at every scale (including all coordinates, keys, normals, indexes, bounds and totals), independent closed-form counts, explicit truth-table membership sets, canonical prism outward triangle normals, the sloped polygon and near/far surfaces, and malformed identity/story/group/frame/shell/opening refusal. No Windows rendering measurements can result from these pure data tests. WB-02 async loading/batching, WB-03 picking, WB-04 visibility/selection and WB-05 authoritative project loading remain separate issues.
