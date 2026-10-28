# APP-01 prototype: where it stands, and the migration gates (2026-10-28)

**Decision status: migration HOLD.** EtabExtension #467 stays HOLD and Tauri/React
stays production. This page sums up the APP-01 prototype (#49) and lays it
against the native-migration gates in [`programme.md`](programme.md) and
[`p0-verification.md`](p0-verification.md).

It is a **record, not a verdict**. Statuses are the owner's recorded acceptance
where one exists. Everything else is an **agent report** from a Linux container
with software Vulkan (lavapipe) and no Windows. Nothing here passes a gate on
its own.

## The prototype slices

| Slice | What it delivers | State | Evidence | Open before the slice is done |
|---|---|---|---|---|
| APP-01A (#50) | Tab-first TALOS shell; typed project/version/workspace state; load tickets refusing stale responses | **Merged** (`e1aea4d`) | Gates in the PR; owner Windows screenshots | Windows DPI 125/150 %, Narrator |
| APP-01B (#53) | Shared wgpu renderer (`lab-viewport`) in Model; one selection authority; GPU lifetime | **Merged** (`25ad580`) | 153 tests + 33 GPU (lavapipe) at `0e7330b`; owner screenshot of picking | Windows occlusion, Ctrl add/remove, v1↔v2, `native_e2k`/`native_workbench` WebView2 smoke |
| APP-01A.2 (#54) | Visual system: one header row, floating/docked panels, one panel when compact, version strip | **Merged** (`816fa22`); owner accepted the visual direction | 164 tests + 44 GPU at `72d7b98`; owner wide/compact screenshots | Exact-head Windows smoke the owner listed (compact pick, strip/header sync, Results flow) |
| APP-01C native (this PR) | Cached results via `lab-results`; story chart + story table; version folders read-only; fail-closed story range; stored Mx/My | **Draft PR** | Gate at this PR's head (see `app-01c-results.md`); owner's real folder checked locally, never published | Owner review; Windows build of the moved `native_results`/`native_workbench` files |
| APP-01C ECharts / WebView2 | The interactive chart in Results through the existing Wry child | **Not started** | — | Windows-only: typed version/fingerprint/generation messages, stale-event denial, focus/z-order/resize, coexistence with the wgpu viewport |
| APP-01D | Authoritative `ext-api` read-only project/version routing | **Blocked** | — | INT-01 (#9/#18) Windows CRT/linker/packaging hold |

## Migration gates

| Gate (programme) | Recorded state | What APP-01 added | What still decides it |
|---|---|---|---|
| #5 native UI | PARTIAL | A working tab-first app on egui 0.37: one-row header, overflow, floating/docked panels, one panel when compact, accessible tabs, panels and rows (AccessKit-asserted), Inter/Lucide, light/dark | Windows 125/150 % DPI, Narrator, keyboard focus under a real window manager, docking, cold restart |
| #6 native 3D | PARTIAL | One shared renderer; depth; CPU nearest-visible picking with scoped selection; GPU tests on lavapipe; owner Windows screenshot of real GPU picking | Windows occlusion/Ctrl/version matrix, device loss/recovery, real-GPU captures at all DPIs |
| #4 performance | PARTIAL (CPU 1k/10k/50k/100k superlinear: FAIL; CPU pick latency: PASS) | None measured | ≥ 3 Windows Release presented-frame runs, CPU/GPU/RSS/VRAM, Three.js reference |
| #2 embedded chart (WebView2) | PARTIAL (hidden WebView2 smokes, #17) | Results is now chart-first with a native chart and table over real cached data; the WebView2 slice is designed but not built | Offline child in TALOS: both selection directions, focus/IME/DPI/z-order/docking, zero network, process cleanup |
| #7 E2K fidelity | PARTIAL (24/24 frames, 19/19 shell labels vs the pinned `ext-api` classifier) | The real model renders with fidelity tiers shown; a story-set mismatch between the model and the cached results is flagged | Engineering-grade geometry (insertion/offset placement, thickness), wall/slab rule (#349) |
| #8 desktop runtime | NOT TESTED | Read-only CLI folders; nothing is written | Packaged install/upgrade/rollback, slow/cancelled jobs, clean shutdown |
| #9 read-only API | BLOCKED (backend CRT/packaging) | Version identity read from folder manifests through the pinned `ext-core` type, labelled as not repository state | APP-01D on `ext-api`, identity/outcome parity, non-mutation |
| #10 calculations | NOT TESTED (parity) | No recalculation by design: cached `CalcOutput` is read through the pinned `ext-calc` types, units/identity checked, values shown as stored | Authoritative synthetic result/unit/sign/envelope parity; the overturning-moment convention |
| #11 charts | NOT TESTED | Stored profiles (drift, displacement, shear, Mx/My) as chart + table with gaps, signs and references kept | Canonical chart-spec/static/interactive parity with `ext-render`, offline assets |
| #12 reports | NOT TESTED | — | Typst/PDF content, retry, artifacts, non-mutation |
| #13 sidecar | NOT TESTED | No ETABS start, ever; "ETABS not connected" status | Simulator, identity, sticky refusal, one owner, timeouts, shutdown |
| Toolchain | PARTIAL | Rust 1.99.1 + pin `9eb1c87`, unchanged throughout | EtabExtension BUILD-06 documents 1.96.0 (#467); unresolved |
| **Migration readiness** | **BLOCKED / HOLD** | A credible native application prototype exists and reads real engineering data safely | Independent acceptance of every gate, an owner-approved ADR, explicit migration approval |

## Conclusion

- **What the prototype shows.**
  - A native egui/wgpu application can carry the TALOS workflow:
    project → version → 3D model with source-backed picking and fidelity →
    cached engineering results as chart and table.
  - It keeps the safety rules: read-only, no ETABS, stale responses refused,
    nothing invented, results labelled as cached and freshness-unverified.
  - The owner's own analyzed version folder loads and plots end to end, locally.
- **What it does not show.**
  - Migration readiness: there is no measured performance, packaging,
    authoritative `ext-api` routing, report path or sidecar.
  - The Windows evidence is limited to the owner's screenshots so far.
  - The WebView2 chart is still to be built.
- **Recommended order from here:**
  1. Owner Windows smoke at exact heads (A.2 list, then APP-01C).
  2. APP-01C ECharts/WebView2 slice, Windows-tested.
  3. Resolve INT-01's CRT/packaging hold, then APP-01D.
  4. Performance (#4) on Windows Release.
  5. Packaging (#8).
  6. Only then a Go/No-Go ADR.
