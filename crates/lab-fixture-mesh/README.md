# WB-02-A: validated synthetic CPU mesh conversion

[PR42](https://github.com/tadoEng/egui_test/pull/42) is the first CPU-only slice of [WB-02 #24](https://github.com/tadoEng/egui_test/issues/24). `lab-fixture-mesh` consumes only immutable validated authored `lab-fixture::Fixture`. It reuses existing control points, prism face rings and shell normals; no parser, engineering calculations, worker, application dependency or GPU resource is introduced.

## Certified source and scope

Measured source: `dfdb302666f2a80d3ac678a6475b8bcd1c6eeaa7`, tree `0bb6600d06c71ee50ccecaa0d002505743ea68bb`. History-preserving merge `79aa3fa` incorporates owner-merged main `6f622918791c2703ace42dd28e15c117eafef13d`. Relative to that main, the manifest adds this workspace member and the lock adds this package record only. All existing feature/dependency records and the authoritative backend pin `9eb1c8792a1d41f3f81c60428afe89f1655782ba` remain intact. Documentation/evidence follow-up changes do not modify compiled inputs.

**PASS for recorded CPU conversion checks; draft pending independent exact-head review.** All native canonical checks passed: fmt, all-target/all-feature check, strict workspace Clippy, plain Cargo **197 passed / 12 ignored**, and four workspace doc suites with zero doc tests. Mandatory all-target/all-feature WASM passed separately with `RUSTC_WRAPPER` unset only in the child, using the documented Windows sccache os206 workaround. This is a recorded split, not a one-shot canonical-script PASS. Additional nextest discovered the same **209 test names and 12 ignored names**, and passed 197 with 12 skipped. It does not replace Cargo/docs or execute ignored GPU probes.

[Final receipt and replayable harness](../../docs/native/evidence/wb02-conversion/final-certification/windows-validation.json), [Cargo/nextest parity](../../docs/native/evidence/wb02-conversion/final-certification/nextest-parity.json), [actual manifest CSV](../../docs/native/evidence/wb02-conversion/final-certification/manifest.csv), [actual Release CPU observations](../../docs/native/evidence/wb02-conversion/final-certification/cpu-budget.csv).

The serialized campaign used Rust1.99.1, sccache0.19.0, nextest0.9.150, locked/offline commands, jobs1, test-threads1, debug0, stack268435456, the physical worktree's own `.codex-target`, and an owned D: cache/temp/server. Launch required 6GiB available RAM and disk; runtime required 2GiB RAM and 6GiB disk with fail-closed sampling. No resource guard stopped the campaign. Cleanup reverified and stopped the current owned listener; shared servers/counters and useful cache entries were preserved. No installation, global flags/profile/CRT changes or owner-app closure occurred.

## Geometry and allocation contract

Prism and actual inclined OrientedPrism become flat-shaded surfaces. Shells remain declared zero-thickness authored approximations. Every generated triangle must remain finite/nondegenerate with consistent normal/winding after recentering and f64-to-f32 conversion. Unsupported openings and invalid surfaces fail rather than silently filling a hole. Existing LabId/story and contiguous index ranges are retained. These labels/IDs are synthetic, never ETABS authority.

One 36-byte position/normal/color vertex stream and one u32 index stream replace per-entity buffer plans. Default triangle-aligned shared-index ranges contain at most300,000 indices and can split within an entity, using base vertex0. They are **planned CPU ranges, not actual GPU draw calls**. The256MiB default checks logical stream lengths before reservation. It does not bound fixture/index-map allocations, allocator capacities, process memory, GPU residency or driver VRAM. Main vectors use fallible reservation.

Lab formula revision is separate from SHA256 identity of the exact origin/vertex/index representation. Canonical hashing includes a mesh-format domain, f64 origin and count boundaries. Repartitioning batches preserves geometry identity; selection must still pair LabId with the immutable lab revision. There is no metadata editing API or camera-dependent triangulation.

Tests cover all authored presets, independent per-kind grid/topology formulas, bounds, complete entity/batch ranges, shell front-side normals, outward prism triangles, actual inclined swept corners, revision/hash/partition identity, exact byte limits, invalid batch limits, unsupported holes, nonplanarity, nonfinite/degenerate surfaces and post-cast collapse.

| Preset | Structural entities | Vertices | Triangles | Logical bytes | Planned ranges |
|---|---:|---:|---:|---:|---:|
| TruthTable |24|437|219|18,360|1|
| Thousand |1,035|15,180|7,590|637,560|1|
| TenThousand |10,281|197,064|98,532|8,276,688|1|
| FiftyThousand |50,715|975,660|487,830|40,977,720|5|
| HundredThousand |99,981|1,919,304|959,652|80,610,768|10|

## Reproduction and CPU observations

```powershell
./check.ps1 -Offline -SkipWeb
# Mandatory split WASM step: unset RUSTC_WRAPPER in this child/session only.
cargo check --workspace --all-targets --all-features --target wasm32-unknown-unknown --locked --offline --target-dir .codex-target
cargo run -p lab-fixture-mesh --example fixture_mesh_manifest --locked --offline --target-dir .codex-target
cargo run -p lab-fixture-mesh --example fixture_mesh_cpu_budget --release --locked --offline --target-dir .codex-target
```

Use the archived guarded harness with process-local owned cache/temp and relative `CARGO_TARGET_DIR=.codex-target`; see its [replay notes](../../docs/native/evidence/wb02-conversion/final-certification/README.md). Four harness files are byte-exact archives. The two launch wrappers have only local path bindings parameterized; original/published hashes and mechanical normalized equivalence are recorded. The modified wrappers were not literally executed.

The CPU utility makes three serial observations after one warmup per preset in **one process**, using the existing Release opt-level2. For99,981 entities, generation including validation observed41.5450–43.6706ms; conversion including triangle checks and SHA256 observed117.0377–117.5560ms. These are one-machine CPU pipeline observations, not independent performance trials, isolated triangulation time, GPU upload, presented frames or FPS. Other owner apps stayed open; compiler and OS caches were not cleared. Approximate process-tree sampling also excludes separate compiler-cache workers and cannot measure VRAM.

Async latest-generation loading/cancellation, last-good retention, bounded actual GPU upload/draws, visible100k scenes and resource lifecycle belong to the separate WB-02-B draft. Picking #25, controls #26, source fidelity #7 and matched presented-frame benchmarking #4 remain open. All six migration gates remain open, Tauri stays production and #18 CRT/package HOLD is unchanged. No private input/derived public evidence, production edit or merge is authorized.

Historical receipts remain unchanged: [focused partial validation](../../docs/native/evidence/wb02-conversion/windows-validation.json), [resource stop](../../docs/native/evidence/wb02-conversion/resource-stop.json), [earlier source reconciliation](../../docs/native/evidence/wb02-conversion/main-reconciliation.json). They are superseded only for this CPU conversion certification.
