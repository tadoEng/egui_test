# Native feasibility programme: execution record

Status: **PARTIAL**. This is an experimental programme, not migration approval.
Tauri/React remains production. No merge or production change is authorized.

## Reviewed requirements and frozen baseline

Read tracker [#14](https://github.com/tadoEng/egui_test/issues/14) and all its linked
issues: [#2](https://github.com/tadoEng/egui_test/issues/2),
[#4–#9](https://github.com/tadoEng/egui_test/issues/9), and
[#10–#13](https://github.com/tadoEng/egui_test/issues/13), including the
2026-10-26 verification addenda, before beginning these implementations.
Issue bodies, rather than historical baseline paragraphs, govern acceptance.

| Source | Exact revision | Role |
|---|---|---|
| egui_test main | `703000b967a6626c0255b2c173e44e5b320eef0a` | PR #3 base |
| egui_test draft PR #3 | `61f6b74adbbd38255a5736fb0de7f432a3e82cd4` | All initial track branches start here |
| EtabExtension | `9eb1c8792a1d41f3f81c60428afe89f1655782ba` | Single backend pin; source is read-only |
| SolidCraft | `4f2c133d01d1b3d8bae026af2ae76b5c677949ac` | Source architecture reference, not dependency |
| Storytold craft-libs | `b9ba47f58902e839a8f102b824bb9f0beaac335c` | Source architecture reference, not dependency |

At the baseline review, PR #3 was draft/unmerged. The reviewer reran `./check.ps1 -Offline` on its
exact head on Windows: formatting, native all-target/all-feature check, strict
Clippy, **23 tests**, doc tests (zero), and WASM all-target/all-feature check
passed. `git diff --check` passed; the checkout remained clean. The independent
review found no actionable defect within that baseline's advertised scope.
This was a separate review pass; it does not substitute for owner review.

The tested scope is CPU-projected Painter wireframes, cached synthetic scenes,
headless interaction, and optional synthetic E2K fixtures. No native-window
interaction, OS DPI event, rendered-pixel, present-time, GPU timing, RSS/VRAM,
engineering-fidelity, or packaged-app acceptance follows from this PASS.
The earlier single-run CPU measurements are not FPS or a capacity ceiling.
Environmental incremental-cache hard-link warnings did not bypass Clippy.

## Parallel tracks and first PR boundaries

All initial branches started from PR #3's branch, with that dependency stated
in each draft PR. They do not change EtabExtension production.

GitHub now records PR #3, the programme document PR #15, and WebView PR #17 as
merged on 2026-10-26. Those merges occurred outside this implementation run;
this run did not issue merge commands. PR #16 and PR #18 remain draft/unmerged.
These repository merges do not establish acceptance of any native gate.

| Track | First smallest implementation | Explicit follow-up boundary |
|---|---|---|
| INT-01 #9 | Native-only optional ext-api adapter; authoritative read-only status/branches/versions/overview/diff; bounded worker; synthetic parity and stale-result tests | Scene/provenance integration after adapter verification; no mutation/sidecar access |
| GATE-02 #6 | Synthetic mesh core with normals, cached vertex/index/uniform buffers, depth, orbit/fit/resize, stock egui-wgpu callback | World picking/occlusion in second PR; reliability and performance in third |
| GATE-04 #2 | Standalone Windows Wry child with locally bundled ECharts, typed story-selection messages, bounds and lifecycle | Dock/focus/IME/DPI matrix next; cross-selection after renderer and backend identities |
| Foundation #14 | This baseline, dependency sequence, source studies, and acceptance ledger | Update with independently checked PR heads and evidence |

Dependency order:

1. Verify PR #3 and source contracts. Keep the backend pin fixed.
2. Run #9, #6 core, and #2 Stage A independently. Study #5 docking and #8 host
   ownership before adopting a UI library or designing a host service.
3. #7 compares the native preview with same-pin ext-api scene/provenance.
   #6 picking uses stable keys and authoritative fidelity disclosures. #4 uses
   matched synthetic geometry/camera across Painter, custom GPU and Three.js.
4. Prove #2 lifecycle/docking first, then connect 3D ⇄ Rust ⇄ chart selection
   with project/version/source identity and generation guards.
5. #10 typed calculations → #11 chart representation; #12 report output follows
   validated fixtures. #8 runtime framework precedes #13 sidecar mocks.
6. Packaged Windows offline review workflow, installer/upgrade/rollback, and
   independently checked gates precede an owner-reviewed Go/No-Go ADR.

No dependency on a draft means its gate has passed. No automatic closure of
issues or acceptance of N/A items. No automatic continuation into real ETABS.

## First published slices

| Track PR / follow-up | Exact reviewed head | Current evidence / assessment |
|---|---|---|
| [#16: native GPU core](https://github.com/tadoEng/egui_test/pull/16) | `57c22dabc3aabd6d4a8e864ff01de02d697329bc` | PARTIAL: 23 baseline + 3 CPU tests and full offline gate pass; explicit RTX 3050 Ti/Vulkan depth/buffer-reuse probe passes; Windows release builds. Fit contains all fixture vertices in tall/wide viewports. No native-window interaction, picking or presentation performance acceptance. |
| [#17: offline WebView child](https://github.com/tadoEng/egui_test/pull/17), merged at `7e4a917` | `29f0747e0a10dfa0c13d36bce48835f8bfdbb620` (evidence follow-up) | PARTIAL: 23 baseline + 11 tests and full offline gate pass; release builds; three fresh hidden WebView2 156.0.4314.42 two-way message smokes pass. Manifest binds binary/reports to runtime source commit `7e4a917`; this follow-up adds evidence/byte policy only. Manual interaction, docking/DPI, network capture and performance remain unverified. |
| [#18: read-only API adapter](https://github.com/tadoEng/egui_test/pull/18) | `6336023c37777ee9c98fb086cbe78aad498063f9` | PARTIAL: full offline gate and all 28 tests pass, including branch history before version selection, DTO parity, non-mutation and stale/busy handling. Unconditional closure adds 503 lockfile records; first check 29m 22s, first test codegen 63m 40s. LNK4098 CRT warning remains unresolved; release/package and engineering parity unverified. |

Each track links its source map, tests, measurements, limitations and remaining
criteria. Later commits supersede the listed head only when explicitly checked
and recorded in that PR. All remain unmerged and pending independent/owner
review; none is a gate PASS or migration approval.

One runtime defect was found and corrected before publishing #17: WebView2
reported its bundled HTML bootstrap as a base64 data URL, so allowing only
about:blank blocked the document. The corrected policy allows the exact
authored bootstrap bytes, with a typed ready handshake. It still rejects
arbitrary data/external navigation; the small success reports preserve the
limited scope of programmatic testing.

## Source reuse and compatibility findings

| Capability | Classification | Source evidence and decision |
|---|---|---|
| SolidCraft render/UI boundary | Adapt pattern | [`ui-egui/src/gpu.rs`, `install`, `ViewportCallback`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/ui-egui/src/gpu.rs): resources owned by GPU renderer; thin prepare/paint callback; same core can support offscreen captures |
| SolidCraft cached batches | Learn/adapt | [`gpu/src/view.rs`, `Batches`, `Renderer::prepare`, `Renderer::paint`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/gpu/src/view.rs): cache buffers independently of UI repaint |
| SolidCraft CAD selection/kernel | Reference only / not needed | [`ui-egui/src/viewport.rs`, `pick_hits`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/crates/ui-egui/src/viewport.rs): geometric picking ideas transfer, CAD domain types do not establish structural fidelity |
| SolidCraft integration stack | Compatibility adaptation | [`Cargo.toml`](https://github.com/tangency-labs/solidcraft/blob/4f2c133d01d1b3d8bae026af2ae76b5c677949ac/Cargo.toml): egui 0.36 vs lab 0.37; both lockfiles resolve wgpu 30.0.1. Its patched egui-wgpu is not imported |
| craft-ui layout/validation/protected viewport | Compatibility candidate | [`docking/model.rs`, `Permissions::PROTECTED`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/ui/src/docking/model.rs): useful source patterns; no adoption before consumer compile/lockfile review |
| craft-ui native floating windows | Compatibility adaptation | [`workspace Cargo.toml`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/Cargo.toml): current head now uses egui **0.37**, superseding issue #5's historical 0.36 note, but patches egui and winit to Storytold forks; consumer patches do not propagate automatically |
| craft-settings | Reference/adapt | Existing production data roots/schema must win; no silent switch to a library's settings policy |
| craft-atomic | Future direct-reuse candidate | For independent host files only; project state/commits remain owned by ext-api/ext-db |
| ext-api read-only construction | Direct reuse | [`context.rs`, `AppContext::new_read_only`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/context.rs): avoids startup cleanup that ordinary construction performs |
| ext-api scene identity and load generations | Direct reuse | [`model_inspect/packet.rs`, `SceneLoadGuard`, `model_scene_packet`, `entity_source_checked`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/model_inspect/packet.rs): version/fingerprint and source-citation rules remain authoritative |
| ext-api dependency closure | Measured compatibility/build concern | [`ext-api/Cargo.toml`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/Cargo.toml): calc/render/report and their Polars/Typst dependencies are unconditional; disabling default features cannot trim them |
| ext-tauri host ownership | Reference only | [`state.rs`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/state.rs), [`lib.rs`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/lib.rs): preserve single validated client, refusal latch, generations, scheduling, menus/events, data roots and shutdown; do not import Tauri as the native backend |

SolidCraft and craft-libs declare MIT OR Apache-2.0; EtabExtension supplies an
Apache-2.0 license. Initial code should adapt architecture rather than copy CAD
or vendor implementation. Any later copied source/assets require file-level
attribution and retained notices, including ECharts' upstream license/NOTICE.
Track-specific source maps expand the inventory and distinguish inspected
source from compatibility tests actually executed.

## Acceptance ledger at programme start

| Gate / integration | Status | Evidence still required |
|---|---|---|
| PR #3 advertised headless baseline | PASS (reviewed at exact head) | Native interaction and fidelity are outside this baseline |
| #5 native UI | PARTIAL | Docking adoption, corrupt layout recovery, keyboard/accessibility, cold restart, actual 100/125/150% DPI and WebView composition |
| #6 native 3D | NOT TESTED | Mesh/depth core, stable occluded picking, real Windows captures and device recovery |
| #4 performance | PARTIAL | Matched 69-storey fixtures; ≥3 Windows Release presented-frame runs, CPU/GPU/RSS/VRAM, pick latency/accuracy, Three.js reference |
| #2 embedded chart | NOT TESTED | Offline child, both selection directions, focus/IME/DPI/z-order/docking, zero runtime network and repeated responsiveness measurements |
| #7 E2K fidelity | PARTIAL | Same-pin ext-api semantic/provenance comparison; warnings, axes, units and geometry truth table; engineering-grade geometry remains unverified |
| #8 desktop runtime | NOT TESTED | Full ownership replacement, slow/cancelled jobs, packaged Windows install/upgrade/rollback and clean shutdown |
| #9 read-only API | NOT TESTED | Typed identity/outcome parity, non-mutation, worker and stale-result proof, dependency/build cost |
| #10 calculations | NOT TESTED | Authoritative synthetic result/unit/sign/envelope parity |
| #11 charts | NOT TESTED | Canonical spec/static/interactive data parity and offline assets |
| #12 reports | NOT TESTED | Synthetic Typst/PDF content, output failure/retry, artifact handling and version non-mutation |
| #13 sidecar | NOT TESTED | Simulator, identity validation, sticky refusal, one owner, queue/timeouts/disconnect/shutdown; no default ETABS launch |
| Migration readiness | BLOCKED | Independent acceptance of every gate plus owner-approved ADR and explicit migration approval |

Statuses distinguish **reviewed evidence**, **implementation-agent reports** and
**hypotheses**. Compilation alone cannot pass a gate. Offscreen GPU readback can
validate pixels/depth; it cannot establish native window focus, OS DPI, display
presentation timing, or installer behavior.

## Evidence and safety protocol

- Synthetic inputs only in commits, PRs, issue comments, screenshots and traces.
  No private model, filename, path, counts, warnings, coordinates, timings or
  other private-derived metadata in public artifacts.
- Default experiments never launch ETABS/COM, write user projects, or issue
  Analyze/Save/Checkout. Fixture initialization is explicitly isolated to temp
  test setup; committed versions remain immutable during reads.
- One reviewed backend SHA in every imported git crate and Cargo.lock. A pin
  change needs a separate diff/changelog/regression review.
- Work runs outside the paint loop; late results cannot replace another
  project/version or source citation. Preserve typed errors and operation IDs.
- Preserve sidecar binary validation, refusal evidence, operation gates and
  explicit ETABS consent when that later track begins. A read-only workflow
  never implies consent to start an external process.
- No Computer Use. Automated CLI/headless/offscreen checks are permitted;
  manual Windows input/focus/DPI/capture requirements remain NOT TESTED until
  the owner supplies or explicitly authorizes an appropriate evidence route.
- Every implementation: small feature branch → draft PR, exact base/head,
  changed files, source references, flags/lockfile, synthetic tests, Windows
  evidence, measurements, limitations and PASS/PARTIAL/BLOCKED assessment.
- No merge without the owner's explicit instruction. Tracker updates record
  evidence; they do not silently tick acceptance boxes.
