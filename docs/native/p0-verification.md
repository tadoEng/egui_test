# Hybrid native migration — P0 verification and gap assessment (2026-10-28)

**Decision: HOLD.** No production migration, no merge. Every result below is
**agent-reported** from one Linux cloud container and has **not** been
independently reviewed. Nothing here was executed on Windows, on a GPU, or
against the Tauri application.

Private model files were evaluated **locally only**. No client-derived name,
path, count, coordinate, timing, screenshot or diagnostic appears in this
repository; private results are retained for the owner and summarized here
only as "local run executed" where relevant.

## Evidence identity

| Item | Value |
|---|---|
| egui_test base | `6f622918791c2703ace42dd28e15c117eafef13d` (`main`, after #44/#45) |
| Branch / head | `claude/funny-cori-cweovj`; gate-verified code head `20703af4150ad4aa45b58aa1309289a5841d2cbd` after review fixes (first review at `aebd019`; later commits are documentation/evidence only) |
| Backend pin | EtabExtension `9eb1c8792a1d41f3f81c60428afe89f1655782ba` (unchanged) |
| Production reference | EtabExtension HEAD `04d67651dd2fd96140f65634905ae3b4b807d663`; `crates/ext-e2k/src` and `ext-api/.../shell_openings.rs` are byte-identical to the pin |
| `Cargo.lock` / manifests | unchanged (sha256 `8e98fe9c…4c83ba`) |
| Toolchain | Rust 1.99.1 (`rust-toolchain`) |
| Host | Linux 6.18 container, 4 vCPU, 15 GiB, **no GPU/Vulkan/EGL, no Windows, no WebView2** |
| Build settings | `--locked --offline -j 2`, worktree-owned `.codex-target`, clean detached worktree per gated SHA; sccache and nextest **not installed** here (not installed by the agent) |

### Gate receipts (`evidence/p0-verification/gate-*.txt`)

| SHA | check | fmt | clippy `-D warnings` all-features | test all-targets all-features | doc | wasm |
|---|---|---|---|---|---|---|
| `fc6b66d` (first fix only) | 0 | 0 | **101** (pre-existing, see G-ENV-1) | 0 — 84 passed, 0 failed, 6 ignored | 0 | 0 |
| `97cd098` (first review) | 0 | 0 | **101** (same pre-existing) | 0 — 102 passed, 0 failed, 6 ignored | 0 | 0 |
| `20703af` (code head, review fixes) | 0 | 0 | **101** (same pre-existing) | 0 — **105 passed, 0 failed, 6 ignored** | 0 | 0 |

Every target except `examples/webview_stage_a` was additionally linted with
strict Clippy at `97cd098` and again at `20703af`: **clean**. The 6 ignored tests
are the opt-in GPU probes; they were **not executed** (no adapter).

**Windows compile: NOT TESTED here — owner runs it on Windows.** A Linux cross
type-check for `x86_64-pc-windows-msvc` was attempted with owner permission and
abandoned: `ring`, `stacker` and `zstd-sys` build scripts need MSVC/Windows SDK
headers, and the `real-workbench` feature pulls `ext-calc`→polars→zstd. No
Windows evidence is claimed. Commands for the owner are in §6.

**G-ENV-1 — recorded narrowly scoped exception (pre-existing, not this branch):** on non-Windows targets
`examples/webview_stage_a.rs:393` uses `eprintln!`, which the workspace
`print_stderr` lint rejects. The file is untouched by this branch and the
canonical gate is Windows `check.ps1`, where that branch is not compiled. A
one-line `#[expect(clippy::print_stderr, reason = …)]` would fix it; left for the
owner because it is outside P0 scope.

## 1. Current-state assessment (what genuinely works)

Verified by executable tests in this session unless stated otherwise.

* **E2K → mesh (CPU).** `lab-e2k-mesh` parses only through pinned `ext-e2k`,
  converts units to metres, recentres in f64 and emits one shared vertex/index
  stream with per-entity ranges and typed withheld reasons. 30 unit tests.
* **Before this branch** (root causes of the reported real-model gaps):
  1. *All walls and slabs missing.* Pinned `ext-e2k` keeps an absent
     `AREAASSIGN … OPENING` key as `None`; the native mesh withheld every such
     shell as `UnknownOpeningState`. `ext-api` treats only an explicit `Yes` as a
     void and draws these shells. The global `has_void` defect was real too, but
     only bites models that contain at least one opening.
  2. *Fixed 100 mm proxies.* The mesh ignored source `depth/width/section/
     material/cardinal/insertion` that `ext-e2k` already resolves.
  3. *Concave slabs missing.* Only strictly convex rings were triangulated.
  4. *No member picking.* Only whole-story index highlighting existed.
* **After this branch:** source D×B frames in CSI default local axes; frame
  fidelity and opening-host labels that **mirror the pinned `ext-api`
  classifier exactly** (checked against the real classifier, below); undeclared
  shells drawn and disclosed; concave planar rings ear-clipped; CPU
  nearest-visible-surface picking with revision+generation-scoped selection,
  hover, Ctrl/Cmd toggle, empty-click clear and a source-attribute inspector.
* **Authoritative parity oracle.** A scratch harness (sources in
  `evidence/p0-verification/`) runs the real pinned `ext-api` path
  (`init_project` → `commit_version{no_e2k, analyze:false}` → snapshot →
  `model_scene_packet`), never launching ETABS, and diffs its per-entity
  `SceneFidelityEntry` list against the native labels. On the public fixture
  set (7 fixtures, incl. a cross-story near-coincident void): **24/24 frames identical (tier and ordered reasons), 19/19 shell
  void/host labels identical, 1 intentional native-stricter case** (a shell that
  partially overlaps a declared void: `ext-api` draws it, the native viewer
  withholds it rather than paint over the void). The oracle found one real
  divergence (reason ordering), which is fixed. A local run on the private model
  was executed; results stay local.
* **Not working / not present:** shell thickness extrusion and shell insertion
  offsets, cardinal/joint-offset placement, section rotation (`ANG` is not
  parsed upstream), hole subtraction, GPU ID-buffer picking, hide/isolate
  filters (#26), async loading/cancel (#24), any presented-frame measurement.

## 2. Verification matrix

Status: PASS / FAIL / PARTIAL / BLOCKED / NOT TESTED. Evidence class for every
row is **agent-reported, not independently reviewed** unless marked otherwise.
`unit` = `cargo test -p lab-e2k-mesh`; `vp` = viewport tests run via
`cargo test --example native_e2k --features real-workbench` (the workbench
example itself is Windows-only and compiles nothing on Linux).

### A. Engineering geometry (P0) — #467 §A, GATE-02 #6, GATE-05 #7

| Test ID | Issue | Contract | Reference | Method | Result | Evidence | Gap | Next action | Blocks migration |
|---|---|---|---|---|---|---|---|---|---|
| P0-FR-01 | [#27](https://github.com/tadoEng/egui_test/issues/27) | Section name, material, shape surfaced per frame | `ext-e2k` `FrameSegment`; `ext-api` `SceneFrame` | unit | PASS | `rectangular_source_sections_…` @97cd098 | — | Independent review | No |
| P0-FR-02 | [#27](https://github.com/tadoEng/egui_test/issues/27) | Rectangular D×B correct in metres, depth on local 2, width on local 3 | CSI frame local axes; `ext-api` sweeps D×B | unit (extents in ft→m) | PASS | column X/Y/Z and beam X/Y/Z extents within 1e-5 m | Windows visual capture | Native capture vs Tauri | Yes (until visually compared) |
| P0-FR-03 | [#6](https://github.com/tadoEng/egui_test/issues/6) | Beam, column, brace, inclined local-axis orientation | CSI default (vertical → local 2 = +X; else upward) | unit | PASS | `diagonal_and_inclined_members_…` | `ANG` rotation not parsed upstream | Upstream contract question | Yes |
| P0-FR-04 | [#27](https://github.com/tadoEng/egui_test/issues/27) | Catalog/circle/unassigned/missing never presented as actual | `ext-api` `classify_frame` | unit + oracle | PASS | Symbolic + typed reason + `FixedDisplayProxy`; oracle 22/22 | Circles/I-shapes still a proxy (backend draws centreline) | Profile meshes later | No |
| P0-FR-05 | [#27](https://github.com/tadoEng/egui_test/issues/27) | Cardinal/joint offset/mirror disclosed | `ext-api` cardinal/mirror rules | unit + oracle | PASS (disclosure) / FAIL (placement) | `cardinal_points_joint_offsets_…` | Offsets are disclosed, not drawn | Apply cardinal offsets for Resolved rectangles | Yes |
| P0-FR-06 | [#27](https://github.com/tadoEng/egui_test/issues/27) | Endpoints/world coordinates | `ext-e2k` projection | unit (pre-existing) | PASS | metre centreline, large-origin f64 tests | — | — | No |
| P0-FR-07 | [#27](https://github.com/tadoEng/egui_test/issues/27) | Section change v1→v2 changes geometry + attribution | revision contract | unit | PASS | `a_section_change_between_versions_…` | Only synthetic versions | Real v1/v2 local | No |
| P0-SH-01 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Unrelated opening must not hide independent shells | #467 §A | unit (pre-fix FAIL recorded) | PASS | 4 failing tests at `6f62291`+tests → pass @`fc6b66d` | — | — | No |
| P0-SH-02 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Host label = backend host rule | `ext-api` `shell_openings.rs` | unit + oracle | PASS | 15/15 oracle; strip/edge/perpendicular/degenerate/partial cases | — | — | No |
| P0-SH-03 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Undeclared `OPENING` shells drawn and disclosed | `ext-e2k`/`ext-api` (`None` ≠ void) | unit + local private run | PASS | `openings_and_unknown_flags_…`; inspector `opening not declared by source` | **Owner policy confirmation** | Owner confirms | Yes (until confirmed) |
| P0-SH-04 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Void never filled; overlap never painted solid | #467 §A | unit | PASS | `OpeningVoid`, `OverlapsUnsubtractedOpening` | Native stricter than backend on partial overlap | Owner/backend decision | No |
| P0-SH-05 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Concave/reversed/invalid rings | backend viewer draws concave | unit | PASS | dart area exact, L-shape both windings, bowtie/warped withheld | — | — | No |
| P0-SH-06 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Shell section/material/thickness/insertion surfaced | `ext-e2k` `ShellPolygon` | unit | PASS (attributes) / FAIL (thickness, insertion not drawn) | `shell_source_attributes_…` | Zero-thickness surfaces; by its insertion rule `ext-api` reports `shellInsertionPointIgnored` for TOP/BOTTOM shells with a thickness | Extrude ±t/2 + insertion offset | Yes |
| P0-SH-07 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Wall vs slab by ShellKind vs property family | `ext-api` #349 rule | NOT TESTED | NOT TESTED | — | Native labels only source Floor/Panel | Mirror or consume `ext-api` | Yes |
| P0-SH-08 | [#27](https://github.com/tadoEng/egui_test/issues/27) | Every absent element has a traceable reason | #467 §A | unit + local private run | PASS | typed `Limitation` per entity; local private run executed, results retained locally | — | — | No |
| P0-PK-01 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Click beam/column/wall/slab → correct key | #25 | unit | PASS | `each_family_is_picked_at_its_own_surface` | — | — | No |
| P0-PK-02 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Nearest visible, occlusion, back faces | depth test `cull_mode: None` | unit | PASS | farthest-hit mutation fails 4/6 picking tests | No GPU readback cross-check | GPU ID/readback probe on Windows | Yes |
| P0-PK-03 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Empty space clears; hidden/clipped/withheld never picked | #25 | unit | PASS (predicate) | near/far window, hidden predicate, corrupt ranges | No hide/isolate UI yet (#26) | Wire filters when #26 lands | Yes |
| P0-PK-04 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Hover ≠ selected; Ctrl toggle; multi-select | #25 | unit (state) | PARTIAL | `selection_replaces_toggles_…` | Hover not GPU-highlighted; no physical input | Windows manual | Yes |
| P0-PK-05 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Pick ray == drawn camera (resize/aspect) | shared FOV/near/far | vp | PASS | ray passes through projected vertices at 3 aspects/orbits | **DPI** untested | Windows 100/125/150% | Yes |
| P0-PK-06 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Selection expires on version/load change | `(Revision, generation)` scope | unit | PASS | same bytes + new generation sees nothing | Workbench host change compiles only on Windows | Windows build | Yes |
| P0-PK-07 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Coplanar overlaps | #25 | unit | PASS (disclosed) | deterministic winner + `coincident` count | GPU may show other face | Owner policy | No |
| P0-PK-08 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Inspector shows actual source attributes + fidelity | #467 §A | vp | PASS | `describe()` asserted | No Windows capture | Windows capture | No |
| P0-PK-09 | [#25](https://github.com/tadoEng/egui_test/issues/25) | Zero incorrect selections, deterministic matrix | #25 | unit | PASS on fixture matrix | 6 picking + 2 vp tests | Matrix is small; no physical mouse | Expand + Windows | Yes |
| P0-SH-09 | [#7](https://github.com/tadoEng/egui_test/issues/7) | Conservative overlap fallback is story-scoped (review P1) | `ext-api` host rule is same-story | unit (pre-fix FAIL recorded) + oracle | PASS | `an_opening_on_another_story_never_withholds_…` failed on `aebd019` before the fix, passes @`e32d48f`; oracle cross-story fixture matches | — | Independent re-review | No |
| P0-PK-10 | [#25](https://github.com/tadoEng/egui_test/issues/25) | UNLINKED provenance refuses element picking (review P1) | workbench `Session::linked` | vp + mutation | PASS (gate) / NOT TESTED (Windows host) | `unlinked_provenance_clears_and_refuses_…`; removing the refusal fails it | Workbench host wiring compiles only on Windows | Owner Windows run (§6) | Yes |

### B. Scale and performance (P1) — #467 §B, GATE-03 #4, #24

| Test ID | Issue | Contract | Method | Result | Evidence | Gap | Blocks |
|---|---|---|---|---|---|---|---|
| P1-CPU-01 | [#24](https://github.com/tadoEng/egui_test/issues/24) | Synthetic 1k/10k/50k/100k parse+mesh, counted separately | Release CPU, 3 runs | **FAIL (scales superlinearly)** | `scale-cpu.csv`: ≈5 ms / 115 ms / 5.5 s / 25.5 s; upstream `project_scene` is ≈99 % of it | **Upstream O(n²)**: `ext_e2k::project_scene` calls `frame_section_for` per frame instance, which scans all frame assignments; `story_elevation` rebuilds the story stack per call; `find_point`/`find_frame` are linear. Affects Tauri too. | Yes |
| P1-CPU-02 | [#25](https://github.com/tadoEng/egui_test/issues/25) | CPU pick latency (not click→visible) | 400 rays × 3 runs | PASS vs 100 ms budget (CPU part only) | 100k p95 ≈ 1.0–1.2 ms, p99 ≤ 2.7 ms, max ≤ 6.2 ms; index build ≈ 21 ms | No BVH; presented highlight latency unmeasured | No |
| P1-GPU-01..07 | [#4](https://github.com/tadoEng/egui_test/issues/4), [#24](https://github.com/tadoEng/egui_test/issues/24) | Upload time, presented p50/p95/p99, VRAM/RSS, load/unload, A→B→A, cancel, device loss | — | **BLOCKED** | no GPU/Windows in container; PR #42 is CPU conversion only | WB-02-B async GPU loader not implemented | Yes |
| P1-TAURI-01 | [#4](https://github.com/tadoEng/egui_test/issues/4) | Same scene vs Tauri/Three.js | — | **NOT TESTED** | — | needs Windows + Tauri build | Yes |

Counts per preset (synthetic, authored): 882 / 9,922 / 49,329 / 98,658 source
entities; 0 withheld; triangles 8,484 / 94,864 / 470,148 / 940,296; one shared
vertex/index stream (logical bytes in CSV). These are CPU buffers, **not** GPU
buffers or draw calls.

### C. Embedded Wry/ECharts (P1) — #467 §C, GATE-04 #2

| Test ID | Contract | Result | Gap |
|---|---|---|---|
| P1-WV-01..13 | Offline init, physical hover/zoom/selection, 3D↔chart↔table, signed values/units, case/group/direction, stale IPC, resize/minimize, DPI 100/125/150, focus/IME, popup z-order, docking, process cleanup, network refusal | **BLOCKED** (Wry/WebView2 are Windows-only target deps; nothing compiles on Linux) | Previously merged evidence remains hidden/programmatic only (#17/#31/#44); 3D element picking is not yet bridged to charts/tables |

### D/E. Production parity and release (P2) — #467 §D/§E, GATE-01 #5, GATE-06 #8

| Test ID | Contract | Result | Note |
|---|---|---|---|
| P2-INV-01 | Inventory Tauri surface | PARTIAL | Read-only count at production HEAD: 98 Vitest files under `apps/desktop`, 67 `#[tauri::command]` functions, crates incl. `ext-tauri`. No behavior run. |
| P2-PAR-01..12 | Project open, version identity, branch/dirty, Save→Commit→Analyze, compare, tables, calc/report, Typst/PDF/Excel, shortcuts/a11y, backup/recovery, sidecar lifecycle, consent/cancel | **NOT TESTED** | `ext-api` integration on HOLD (#18 CRT); no native equivalents exist for most rows |
| P2-REL-01..13 | Release build, installer, upgrade/rollback/uninstall, WebView2 runtime, MSVC/CRT, no-ETABS startup, mocked sidecar, crash recovery, cleanup, offline, low mem/disk, logs | **BLOCKED** (Windows) / **NOT TESTED** | — |
| P2-TC-01 | Toolchain/pin compatibility | PARTIAL | This repo 1.99.1 + pin `9eb1c87`; EtabExtension BUILD-06 documents 1.96.0 (#467). Unresolved. |

## 3. Gap analysis

**Geometry:** shell thickness/insertion not drawn (by its insertion rule `ext-api` reports
`shellInsertionPointIgnored` for TOP/BOTTOM shells with a thickness); cardinal and joint offsets not
drawn; local-axis rotation unparsed upstream; non-rectangular profiles are
proxies; hole subtraction absent (both hosts); wall/slab property-family rule
(#349) not mirrored. Native fidelity labels are a **mirror** of `ext-api`
(temporary until #18), so every backend rule change needs a parity re-run.

**Selection:** CPU ray picking only (no GPU ID buffer cross-check); hover not
highlighted on GPU; no hide/isolate filters (#26); no 3D→chart/table bridge;
no physical-input/DPI evidence.

**Performance:** the pinned backend projection is quadratic in frame instances
— the dominant 50k/100k cost for **both** hosts. GPU loading, presented frames,
memory and lifecycle are entirely unmeasured.

**Runtime/parity:** no native project/VCS/calc/report/sidecar workflow; #18 CRT
hold unresolved; toolchain split (1.99.1 vs 1.96.0).

## 4. Prioritized execution plan (small, independent slices)

1. **This draft PR** (P0 geometry + picking) → independent exact-head review;
   owner confirms the undeclared-`OPENING` display policy (P0-SH-03).
2. **Upstream EtabExtension issue (owner-approved, backend repo):** index
   frame/shell assignments, story stack and points once in `project_scene`.
   Acceptance: same `ModelScene` bytes on the committed fixtures; 100k synthetic
   projection linear. Required before any 100k claim on either host.
3. **Shell thickness + insertion extrusion** (mirror the backend viewer's ±t/2
   and `ext-api` insertion rule). Acceptance: oracle parity reports no
   `shellInsertionPointIgnored` difference in kind; area/volume unit tests.
4. **Cardinal-point placement for Resolved rectangles.** Acceptance: offset
   unit tests for points 1–9; oracle reason dropped only where geometry now
   matches.
5. **Windows P0 manual matrix:** physical clicks on each family, occlusion,
   DPI 100/125/150 %, resize, version switch; screenshots from public fixtures.
6. **WB-02-B (#24):** async GPU loader with stale-generation suppression, then
   #4 presented-frame p50/p95/p99 ×3 vs Tauri on the same synthetic scenes.
7. **#26 hide/isolate** wired into the pick predicate; then **#29** typed
   3D↔chart↔table selection.
8. **Replace the native fidelity mirror** with `ext-api`'s classifier once #18's
   CRT/package hold is resolved, keeping the oracle diff as a regression test.

## 5. Go / No-Go

| Gate | Status | Basis |
|---|---|---|
| GATE-01 native UI #5 | **NO-GO (not evidenced)** | no Windows docking/keyboard/a11y/persistence runs |
| GATE-02 native 3D #6 | **PARTIAL — NO-GO** | source D×B, scoped openings, concave shells, CPU picking verified in unit tests; thickness/insertion/offsets, GPU-picking cross-check and Windows evidence missing |
| GATE-03 performance #4 | **NO-GO** | upstream quadratic projection; no GPU/presented measurements |
| GATE-04 ECharts #2 | **NO-GO (BLOCKED here)** | no physical WebView evidence |
| GATE-05 E2K fidelity #7 | **PARTIAL — NO-GO** | native labels match `ext-api` on public fixtures (oracle); remaining geometry gaps above; owner policy pending |
| GATE-06 runtime parity #8 | **NO-GO** | not started natively; #18 hold |
| **Overall migration** | **HOLD** | six gates open, no functional parity, no ADR |

## 6. Owner Windows verification (to run on the owner's machine)

Record the exact PR head SHA, Windows build, Rust version, GPU/driver and
PASS/FAIL per step. Publish only sanitized public-fixture evidence: no private
model source, names, counts, screenshots or timings.

**Code compiled here vs not.** Everything except the Windows-only
`native_workbench`/`native_dashboard` host modules was compiled and tested on
Linux. `examples/native_workbench/host.rs` (the `selection_allowed` wiring and
`--public-workbench-interactive`) has **never been compiled**; step 1 is its
first compile.

### Step 1 — canonical gate and builds

```powershell
$env:CARGO_TARGET_DIR = ".codex-target"
pwsh ./check.ps1 -Offline
cargo build --release --locked --offline --example native_workbench --features real-workbench
cargo test --locked --offline --example native_workbench --features real-workbench
```

### Step 2 — hidden automated smoke (no physical input; window is invisible)

```powershell
cargo test --locked --offline --example native_e2k --features real-workbench -- --include-ignored
cargo run --release --locked --offline --example native_e2k --features real-geometry -- --public-smoke
cargo run --release --locked --offline --example native_workbench --features real-workbench -- --public-workbench-smoke
```

These own and remove their temporary inputs and print a receipt. They prove GPU
upload/readback and scripted chart/UNLINKED behavior only; they are **not**
physical mouse, keyboard or DPI evidence.

### Step 3 — visible physical tests on an owned public fixture

```powershell
cargo run --release --locked --offline --example native_workbench --features real-workbench -- --public-workbench-interactive
```

`--public-workbench-interactive` writes the authored public fixture (synthetic
E2K + synthetic cached results) to an owned temporary directory, opens a
visible window, and removes that directory on exit. It never accepts a user
path and never touches a real snapshot or E2K. At display scaling 100 %, 125 %
and 150 %:

1. Click the beam, column and slab of the fixture (it has no wall; wall
   picking is exercised in step 4 and by the unit/viewport tests): the inspector overlay shows
   that element's key, section/shell attributes and fidelity reasons, and only
   that element is highlighted.
2. Orbit so a beam is in front of the slab; click the beam: the beam (front
   surface) is selected, not the slab.
3. Ctrl-click a second element (both listed); Ctrl-click it again (removed);
   click empty background (cleared). Hover shows a separate "Hover:" line.
4. Resize the window and orbit/zoom, then repeat step 1.
5. Charts: select a story in a chart and in the story list; the 3D story
   highlight and table follow; element selection takes precedence over the
   story highlight while an element is selected.
6. Press **"UNLINK test: invalidate owned public fixture"**. Expected: the red
   UNLINKED notice; element selection and hover clear; the viewport shows
   "UNLINKED preview: element selection disabled…"; clicks and Ctrl-clicks do
   nothing; charts/story selection are disabled. The control restores the
   fixture bytes and rechecks: it must remain UNLINKED. Pressing "Recheck
   selected version" must also remain UNLINKED.
7. Reopen = close the window and run the command again: picking works on a
   fresh, linked fixture.

### Step 4 — visible source E2K picking on a local copy (private, stays local)

Copy one committed version directory to a scratch location first; never point
the workbench at a live project. Repeat step 3.1–3.4 there. Record PASS/FAIL
only; keep everything else local.
