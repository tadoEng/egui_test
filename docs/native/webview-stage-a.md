# Offline Wry/WebView2 Stage A

Assessment: **PARTIAL** for #2 / GATE-04. This standalone Windows example is
stacked on PR #3 head `61f6b74adbbd38255a5736fb0de7f432a3e82cd4`.
It does not change the structural viewer, production, or backend pin.

## Run

```powershell
cargo run --release --locked --offline --example webview_stage_a --features webview
```

Dependencies must be fetched once before using `--offline`. WebView2 Runtime
must already be installed; no installer or automatic runtime provisioning is
claimed. The application has no dev server, React, ETABS, model input or remote
chart data. Its six-story drift fixture is independently authored synthetic
data, **not ext-calc output or demonstrated production chart parity**.

The chart has a drift-ratio axis, named synthetic case, limit line, selected
story, tooltip and inside/slider zoom. Rust owns the selected story; the chart
owns presentation. A native selector and validated chart messages use the same
bounded Story type. Hide, close and reopen controls are implemented; their
Windows focus/DPI/leak behavior is not yet accepted.

## Source and versions

| Component | Version / inspected source |
|---|---|
| Rust / lab | Pinned Rust 1.99.1; egui/eframe/egui-wgpu 0.37.0; wgpu 30.0.1 |
| eframe | Crate VCS metadata `3c6eb8261c3293ad9c7ea1ed58487e038860ebef`; [`epi.rs` native-handle implementations](https://github.com/emilk/egui/blob/3c6eb8261c3293ad9c7ea1ed58487e038860ebef/crates/eframe/src/epi.rs) |
| Wry | **0.55.1**, fixed exact version and registry checksum in Cargo.lock; [`WebViewBuilder`, `build_as_child`, IPC and navigation API](https://docs.rs/wry/0.55.1/wry/struct.WebViewBuilder.html) |
| Wry source identity | Crate metadata names `a5bf203a1c8dbb3583588382538d6521655222a8` and `dirty: true`; the published crate checksum, rather than that commit alone, identifies the inspected bytes |
| WebView2 bindings | webview2-com / webview2-com-sys 0.38.2 in Cargo.lock |
| WebView2 Runtime observed | **156.0.4314.42**, Windows 11 build 26300 |
| ECharts | Apache **6.0.0**; runtime/license/NOTICE bundled locally; [asset provenance and SHA-256](../../assets/vendor/echarts/PROVENANCE.md) |

The safe native handle path is `CreationContext` / `Frame` → Wry
`build_as_child`. Bounds use egui logical points multiplied by the current
pixels-per-point and passed as Wry physical bounds. No unsafe code, forked egui,
native docking library or custom winit host is introduced. See the separate
[Storytold compatibility and host ownership matrix](ui-host-source-map.md).

## Protocol and offline policy

Protocol v1 accepts only:

```json
{"version":1,"type":"chart_ready"}
{"version":1,"type":"select_story","story":3}
```

`chart_ready` is sent after the authored chart and bridge are initialized.
Rust waits for it before sending a selection. Both directions serialize JSON;
the generated script calls a fixed function with a bounded numeric Story. No
user string becomes JavaScript code. Unknown fields/types/versions, malformed
JSON, out-of-range stories and messages larger than 1024 bytes are rejected.
The incoming queue holds at most 16 messages; it never sends on mousemove.
Repeated story selection does not create another transition or echo loop.

The inline page contains the complete verified ECharts asset. CSP denies
connections, frames, objects and forms and permits only authored inline
scripts/styles and data images. New windows, downloads and external navigation
are denied. Devtools and several ancillary browser features are disabled.

An observed Windows failure informed the policy: allowing only `about:blank`
left an empty document because this WebView2 runtime reported the local
`NavigateToString` bootstrap as `data:text/html;charset=utf-8;base64,...`.
The policy now permits **only the exact base64 URL of the generated authored
HTML**, plus the internal blank page. It does not allow arbitrary `data:` URLs.
Negative tests reject a different HTML payload with the same URL prefix.
Navigation diagnostics retain bounded categories, never full page/URL bodies.

These are source restrictions and tests. Runtime network-denial capture with
zero HTTP requests remains **NOT TESTED**; the smoke report says so explicitly.

## Windows programmatic evidence

Three release smoke runs passed on 2026-10-26. Each created its own hidden
eframe/Wry child, waited for the typed ready message, sent Story 5 from Rust,
read back JavaScript's selected story, sent a synthetic chart message for
Story 2, verified Rust's selection and closed the host.

Small raw reports: [run 1](evidence/webview-smoke-1.json),
[run 2](evidence/webview-smoke-2.json), [run 3](evidence/webview-smoke-3.json).
The [run manifest](evidence/webview-run-manifest.json) binds the source commit,
binary/report hashes, UTC capture times and exit codes. Every run uses a fresh
unique report path and rejects missing reports or abnormal process exit, so an
older PASS file cannot be mistaken for a new run.
They contain synthetic state only. These runs are correctness checks, **not
mouse/keyboard input tests, performance runs, presentation timing or leak tests**.

To repeat, build the release example, set `WEBVIEW_SMOKE_REPORT` to a fresh JSON
path, run the EXE and inspect `assessment`. The host stays hidden, has a 30 s
internal deadline and closes itself. Unset the variable for interactive use.

```powershell
cargo build --release --locked --offline --example webview_stage_a --features webview
$env:WEBVIEW_SMOKE_REPORT = Join-Path $PWD ("target/webview-smoke-{0}.json" -f [guid]::NewGuid())
& ./target/release/examples/webview_stage_a.exe
if ($LASTEXITCODE -ne 0) { throw 'Smoke process exited abnormally' }
$result = Get-Content $env:WEBVIEW_SMOKE_REPORT -Raw -ErrorAction Stop | ConvertFrom-Json
if ($result.assessment -ne 'PASS') { throw 'Bridge smoke failed' }
$result
Remove-Item Env:WEBVIEW_SMOKE_REPORT
```

The measured release EXE is 18,564,608 bytes. This is an indicative single executable size, not installed
footprint, RSS/VRAM or a matched host-size comparison. The first partly cached
release build reported 2m 52s while other tracks compiled; later rebuilds reused
that cache. Neither is a controlled clean-build benchmark.

Final `check.ps1 -Offline` passed formatting, native all-target/all-feature
check, strict Clippy, **23 existing tests + 11 WebView/protocol tests**, doc
tests and WASM all-target/all-feature compilation. A separate final Windows
release build with `--locked --offline` also passed. Pure conversion tests
cover 100/125/150% scale factors; they do not simulate actual OS DPI events.

## Acceptance ledger

| Criterion | Assessment and evidence |
|---|---|
| Safe child-host API / release compilation | PASS for compiled Windows source and three actual child-host smoke runs |
| Typed protocol / invalid input / repeated selection | PASS for unit tests and programmatic two-way message smoke |
| Local assets / restrictive source policy | PASS for bundle/hash/CSP/navigation tests; observed zero runtime network NOT TESTED |
| Logical-to-physical bounds conversion | PASS for pure conversion tests; real 100/125/150% OS DPI, monitor moves, resize/minimize NOT TESTED |
| Mouse tooltip, wheel/drag, actual chart click | Implemented; manual Windows observation NOT TESTED |
| Keyboard, IME, Escape, tab traversal, pointer capture | NOT TESTED |
| Hide/show/reopen and process/RSS lifetime | Controls implemented; repeated native teardown/leak validation NOT TESTED |
| Docking, split/float/z-order/overlays | NOT TESTED; child HWND sits above native painted content and may obstruct overlays |
| Project/version/EntityKey cross-selection | Deferred Stage C after #6 and #9; this story-only fixture has no project identity |
| Same-source calculation/chart parity | Deferred #10/#11; no engineering calculation is recreated here |
| Selection latency p50/p95/p99, RSS and responsiveness | NOT MEASURED; no FPS or ≤100 ms claim |
| Installer, runtime deployment, upgrades | NOT TESTED |

The host remains a candidate **child-panel experiment**, not an accepted docking
architecture. If real HWND/focus/floating behavior blocks the workflow, record
it and compare an explicitly scoped custom-winit host in a separate PR. No
silent architecture replacement and no migration-readiness claim.
