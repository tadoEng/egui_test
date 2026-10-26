# DASH-01 first composite host

Issue [#28](https://github.com/tadoEng/egui_test/issues/28), base renderer-main `edab698616d4bee13971c50dcdb0811e26b3a458`. **PARTIAL experiment**, not a migration decision. No backend, project opening, persistence or ETABS automation is used. The default run opens a visible Windows host; test mode is an internally hidden, authored fixture selfsmoke. Computer Use is not part of this work.

```powershell
cargo run --release --locked --offline --features native-renderer,webview --example native_dashboard
```

The dashboard does not instantiate `TemplateApp` and cannot restore its private E2K state. The `native_renderer` and `webview_stage_a` examples remain available with their original run commands. No new package dependencies or EtabExtension pin changes.

## What this slice owns

- One eframe/wgpu native host with a cached solid/depth GPU viewport, story explorer, typed inspector, two KPI cards, native read-only table and **one child WebView with two ECharts instances**.
- Story range1–69, fixture A/B, direction X/Y and mm/kN versus m/N display controls derive all chart/table/KPI values from one Rust packet. Invalid ranges retain the last good state; hiding the selected story resets selection to the first visible story. Clear selection is represented as None.
- Selecting a story in explorer/table sends one cached update to both charts. Chart clicks/ArrowUp/ArrowDown/Escape send typed revision-scoped select/clear messages to Rust. Repeated selections are idempotent; A→B→A increments generation so old events remain stale. No feedback message is emitted by the JS apply operation.
- Child bounds follow physical pixels, hide/minimize and native splitter resizing. Destroy/recreate creates a fresh child/queue/ready flag and replays current state after its ready event. The child remains a fixed panel; floating/docking and popup composition are **not implemented or certified**.

The 3D viewport is the accepted **small independent LAB-CORE-01 boxes/shell preview** from #16, extracted into `examples/support/native_viewport.rs`. The charts/table use **LAB-DASH-01 r1**. These are explicitly separate fixtures: chart stories do not identify 3D entities. No picked elements, geometry isolation, story mesh filtering or 3D cross-highlights are claimed. Mixed 100k GPU loading belongs to WB-02 after #30's contract acceptance; engineering/model parity and cross-selection belong to #29 after authoritative APIs/selection exist. The element explorer portion remains deferred to that geometry integration.

## Fixture and presentation contract

Canonical independently authored signals, **not calculations/results**: for story s1–69, fixture A/X drift = s×0.0001 m, shear = (70−s)×10,000 N. Fixture B multiplies both by2; Y multiplies both by0.5. Rust applies display conversions once (m→mm×1000, N→kN×0.001); JS maps packet numbers directly to series. Synthetic reference lines are0.003m and500,000N, not design limits. Default story1 is0.1mm/690kN; story69 is6.9mm/10kN; peak fixture drift6.9mm. The two charts each have69 points initially; filter10–12 has3 points each. Counts are data points, not structural elements, triangles or GPU draw calls.

Case B/Y, stories10–12 in m/N, yields drift0.001/0.0011/0.0012m and shear600,000/590,000/580,000N. All UI surfaces consume the same cached Rust rows. KPI values are fixture summaries; no JS engineering calculations, ext-calc copy or source-derived results.

Packets/messages carry protocol2, dataset ID, fixture revision and monotonic generation. Only drift/shear plot IDs and visible story IDs are accepted, payloads are bounded to1024bytes, unknown fields rejected. Shared transport retains the exact authored bootstrap navigation allowlist, denied external downloads/windows and incognito policy from #17. Per-frame drain is capped16; selection state never accepts retired contexts. ECharts6.0.0 remains bundled with its existing license/notice; CSP denies page network connections. **Actual runtime zero-network capture is still pending.**

The JS selection update changes only mark lines within the same generation, preserving chart zoom/legend state; context changes replace the options. Tooltips, legend and data-zoom controls exist in the page; their actual mouse/focus/DPI behavior requires manual evidence.

## Validation and remaining Windows evidence

Commands for the exact review head:

```powershell
./check.ps1 -Offline
cargo test --all-features --example native_renderer --locked --offline -- --ignored --nocapture
```

The optional GPU probe validates the shared depth/buffer implementation offscreen; it is not a compositor or actual input test. The dashboard's `DASHBOARD_SMOKE_REPORT` mode validates programmatic chart options/readback, unit/range updates, stale-message refusal, current selection reflection and one child recreation. It cannot prove manual click, presented-frame pacing, native/table/3D entity parity, runtime RSS or child-process shutdown safety.

Manual owner checks once the visible build is reviewable: move/resize splitters and window; hover/legend/wheel/pan both charts; switch A/B/X/Y/units/range and compare table/plots/KPIs; select stories both directions; Escape/keyboard focus; hide/destroy/reopen charts; minimize/restore; repeat at100/125/150% DPI and across monitors; open overlapping native UI and record child-HWND occlusion. Record failures as limitations. No automatic script or screenshot is a substitute for these observations.

Child creation/recreation uses the existing synchronous Wry UI-thread SDK boundary; no nonblocking startup or native responsiveness claim is made. There is one native parent window plus normal WebView2 runtime subprocesses, not one total OS process.

Code source `739daf6` passed full `check.ps1 -Offline`: fmt, strict Clippy, native tests/docs and all-feature WASM. **58 test executions** include 10 duplicated shared regressions across the two example binaries; there are **7 distinct new dashboard tests** (six fixture/protocol cases and one embedded-template case). Both opt-in GPU tests are ignored by the standard workflow; one native-renderer offscreen test was explicitly executed and PASS on RTX3050Ti/Vulkan driver591.74. Fresh64×64 depth readback was inspected (near red square, black background), SHA256 `e2e48afa2c5d4d4d8db6f871230bf70b1c17db41f96ec84acc4741ce4b6b18a2`; it is not a dashboard screenshot.

The owned hidden dashboard selfsmoke PASS covers two actual ECharts instances, 69→3 rows/units/case/direction update, selected-story59 Rust→chart readback, one rejected stale generation, current chart IPC selection11 and one WebView destroy/recreate restoring state. It also requires the native wgpu callback to record at least once (51 callback paint records in this Debug run); **these are not completed/presented frames or FPS**. Refactored Stage A selfsmoke also PASS. WebView2=156.0.4314.42. Reports/source/binary identities are in [the programmatic evidence](evidence/dashboard/windows-programmatic.json); no raw local paths/logs were uploaded. Filesystem incremental-cache hard-link fallback warnings remain visible; no source lint or CRT warning suppressions.

### Release verification

Exact source `a21ba7983ce34a3b97815056d43ecd92b205985c` built all three synthetic examples successfully on a fresh C: target with the guarded runner. Profile: jobs1,opt2,debug0,LTO=false,incremental=false,Rust stack268435456. Cargo process671.126s;6GiB disk/2GiB available-memory guards did not trigger; source/lock/log identities stayed unchanged. This is one build verification with populated global caches, **not a matched performance comparison**. Approximate sampled build working sets are not runtime RSS. No linker warning appeared in these API-free example builds; #18's separate API CRT HOLD remains unresolved.

| Release artifact | Bytes |
|---|---:|
| native_dashboard.exe | 19,067,392 |
| native_renderer.exe | 15,787,008 |
| webview_stage_a.exe | 18,569,216 |

Release hidden composite and legacy Stage A selfsmokes both PASS; the composite recorded46native callback paints and one child recreation. Explicit Release offscreen GPU depth regression also PASS on the same RTX3050Ti/Vulkan driver; its pixels hash identically to the inspected probe above. [Release identities and reports](evidence/dashboard/windows-release.json), [build receipt](evidence/dashboard/release-build.json), [artifact hashes](evidence/dashboard/release-artifacts.json), and all320selected Cargo timing-unit rows are committed. The copied local preview, with source and ECharts license/notice files, also passes from its own folder using this machine's installed WebView2 runtime. This is not validation on a clean Windows machine or a shipping installer. No executable is uploaded publicly.

Dense1/4/12-chart modes, 10k/100k chart points, real docking/float, 20× lifecycle/RSS measurements, device recovery, latency quantiles, runtime HTTP capture, Tauri comparisons and all migration gates remain open. **PASS for recorded programmatic checks; PARTIAL for DASH-01**, pending independent review and actual Windows input/presentation evidence.
