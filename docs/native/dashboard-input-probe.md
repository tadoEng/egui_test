# Optional Windows overlay and text-input probe

Follow-up to [#31 review5652000119](https://github.com/tadoEng/egui_test/pull/31#pullrequestreview-5652000119), [#28](https://github.com/tadoEng/egui_test/issues/28) and [#32's pending matrix](https://github.com/tadoEng/egui_test/pull/32). This branch is independently stacked on unmerged #31 head `84d4a2a05d3e111c558a0f2806576c9adb0f66ba`; it does not require #32's scripts. No merge authorization or migration-gate pass.

The visible `native_dashboard` now offers an **opt-in** top-bar **Windows input probe**. It creates an egui floating window in the **parent HWND**, not a second native window or WebView. Inside are a128-character single-line Unicode text field, its egui focus indicator, Clear probe text and Probe popup. This supplies explicit controls for the previously missing popup/IME test cases. It does not repair or hide child-HWND airspace problems.

- **Probe in native area** places the window on the native left side. **Probe over charts** deliberately places its upper-left corner16logical pixels inside the chart allocation while leaving the WebView visible. These are one-shot placements; the owner may drag the overlay afterwards.
- **Focus probe text**, in the top bar, requests egui field focus. This is not proof that Windows IME or native browser focus transferred correctly. The top-bar controls remain the escape route if the child obscures the entire overlay.
- Text remains only in this in-memory probe. It is never persisted, logged, sent through IPC, included in smoke reports or used as chart/model data. Use synthetic text in any recording. The normal default remains probe closed; existing hidden selfsmokes do not open it.

## Owner-run visible cases

Use the new source/binary identities for this branch, **not** the earlier a21ba79 preview: the old binary does not contain these controls. Build/run the API-free example:

```powershell
cargo run --release --locked --offline --features native-renderer,webview --example native_dashboard
```

Unset `DASHBOARD_SMOKE_REPORT` to run visibly. Do not launch the root viewer or supply project/private inputs. No Computer Use, SendInput or scripted fake manual results.

| Case | Actual user actions | Expected observation to record | Status |
|---|---|---|---|
| P01 | Enable Windows input probe; choose native area; click field and type synthetic Unicode using the physical keyboard. | Field receives input; native story selection and both charts remain correct. Record glyph/readability/focus behavior. | PENDING |
| P02 | With OS IME enabled, compose/commit/cancel text; use arrows/Escape; click chart, then text field again. | Candidate UI placement, composition and focus handoff work without trapped keys or accidental chart commands. No headless test can certify this. | PENDING |
| P03 | Open Probe popup over native content; choose Probe over charts and open/drag the popup/window across the child boundary. | Overlay/menu draws and receives clicks correctly, or capture its actual occlusion/input failure. Keep chart visible throughout; no hide-on-popup workaround is applied. | PENDING |
| P04 | If obscured, use top-bar Probe in native area or uncheck Windows input probe. Then exercise chart→native and native→both markers. | Overlay can be recovered/closed without a ghost or focus trap, and selection still agrees. | PENDING |
| P05 | Repeat P01–P04 at100/125/150% Windows scale and during resize/minimize/restore; multi-monitor transition if available. | Record each scale separately, including popup/candidate position and hit-target alignment. Unavailable hardware stays NOT TESTED. | PENDING |

Use #32's observation format: observer/date, source/EXE SHA, environment/scale, actions, actual outcome, recording/hash/time offset and reproducibility. Existing #31 actual-input cells remain pending. The missing fixture is addressed by source only; W11/W13 do not become PASS merely because a control exists. Dock/float/reparent is still absent. If child airspace prevents correct overlay input, record a blocking composition defect and investigate a separately reviewed host alternative.

## Validation scope

One new **headless egui input-routing test** checks that an unfocused field ignores text and the focused field accepts synthetic Unicode. It uses egui input events in memory, no native window/browser, and explicitly clears unused texture deltas. It does not test an OS IME, physical input, fonts on screen, child focus or compositor z-order.

At code head `0dc88633d020706491d2515891a3354b6686655e`, full `check.ps1 -Offline` **PASS**: fmt, native all-target/all-feature check, strict Clippy, **59 test executions**, docs0 and all-target/all-feature WASM. Ten shared-regression executions repeat across the dashboard/legacy examples; the probe adds one distinct headless test. Both opt-in GPU instances remain ignored; no GPU probe was rerun. Cache hard-link fallback warnings remain visible.

Guarded warm-cache Release build **PASS** at the same code head: jobs1,opt2,debug0,LTO=false,incremental=false,6GiB disk/2GiB available-RAM guard. Source/tree/lock/log identities unchanged; neither guard triggered. This is one17.849s build verification, not a benchmark comparison. Dashboard EXE **19,257,856 bytes**, SHA256 **`4220d01f7c8d29f694d2470bd83085aecdb7c8fd9b2db6870dfd429b7d9ddef9`**. A separate local preview copy plus license/notice files hashes identically. [Sanitized evidence](evidence/dashboard/windows-input-probe.json) records source, test/log and artifact identities. The evidence follow-up changes docs/JSON only.

**No new app execution has occurred.** The preview has not been launched, and the new overlay/IME/compositor behavior is still PENDING owner observation. Earlier #31 programmatic smokes are historical, not rerun evidence for this changed binary. No dependencies, renderer/WebView transport policy, calculation logic, backend pin, project access or smoke-report contents change. #18 CRT/package HOLD, runtime network/RSS/lifecycle/presentation and Tauri parity remain open; Tauri remains production.
