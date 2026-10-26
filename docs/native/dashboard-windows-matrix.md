# DASH-01 visible Windows interaction and compositor matrix

Follow-up to [#31 review5652000119](https://github.com/tadoEng/egui_test/pull/31#pullrequestreview-5652000119), [#28](https://github.com/tadoEng/egui_test/issues/28), [#2](https://github.com/tadoEng/egui_test/issues/2) and [#5](https://github.com/tadoEng/egui_test/issues/5). This verification-only branch is stacked on #31 head `84d4a2a05d3e111c558a0f2806576c9adb0f66ba`. It does not authorize either merge.

**Assessment: PARTIAL. All actual-input observations are PENDING.** Hidden script/readback smokes are accepted programmatic evidence, not visible Windows observations. The owner performs visible steps; no Computer Use, SendInput, SendKeys or JS impersonation of manual input. Record who actually observed each result. All six migration gates remain open; Tauri remains production.

## Fixed test article and setup

Use the API-free `native_dashboard.exe` Release example built at `a21ba7983ce34a3b97815056d43ecd92b205985c`: **19,067,392 bytes**, SHA256 **`71fd13fe1d8692f749ed4f569369e4160398b506084beb3c0f55d61590b3a6e6`**. See [artifact identities](evidence/dashboard/release-artifacts.json) and [Release evidence](evidence/dashboard/windows-release.json). The later #31 evidence-only head has identical compiled inputs. The local copied-preview command is in the chat; keep internal executable/profile paths and binaries out of public artifacts.

1. Record actual executable hash, source, Windows build, GPU/driver, WebView2 version, monitor resolution/scale and input device. Prior runs used Windows26300, RTX3050Ti/Vulkan591.74, WebView2 156.0.4314.42; do not copy those into new results without checking.
2. Launch the **visible example** with `DASHBOARD_SMOKE_REPORT` unset. Do not launch the root `eframe_template` viewer, which can restore previous private input. Use only LAB-DASH-01/LAB-CORE-01. Close the test instance normally; never kill shared WebView2 processes or other app instances.
3. Begin at stories1–69, Fixture A, X, mm/kN, Charts visible, Ready. Two plots have69 points each. Native inspector/table and both horizontal markers reflect the selected story. Vertical references are authored signals, not design limits.
4. Capture short synthetic-only recordings of input/results/defects with the pointer and selected story visible. Exclude unrelated windows/notifications/private files. A static screenshot is not input evidence.

Run W01–W11 at each **Windows display scale100%,125%,150%**, relaunching after static-scale changes. Browser zoom is not display scaling. W12 tests a live monitor transition separately. Record unavailable hardware/control coverage as BLOCKED or NOT TESTED, never PASS.

## Actions and required observations

| ID | Actual user actions | Required visible result / failure to record |
|---|---|---|
| W01 | Hover data points in both plots; compare stories1/69 to table. | Tooltips readable and in bounds. A/X/mm/kN: story1=0.1mm/690kN, story69=6.9mm/10kN; references3mm/500kN. No improper overlay of native controls. |
| W02 | Click a **series point** in drift, then a different point in shear. | Exact story selected in native inspector/table; **both** horizontal markers follow. Record ignored/misaligned clicks or feedback loop. Empty plot click has no specified clear behavior. |
| W03 | With a chart focused, click a native table row, then explorer story. | First click activates/selects native target and both markers agree; no swallowed click, focus trap or stale marker. |
| W04 | Wheel/inside-zoom, drag zoom slider, pan and toggle legend in each plot; then select another story natively. | Only intended chart responds. Selection-only update preserves zoom/legend; native table/3D does not move accidentally. Restore zoom/legend before next step. |
| W05 | Focus each plot, ArrowUp/Down, Escape, then native story. Try native Clear selection and Tab/Shift+Tab. | Arrows select adjacent visible stories. Escape/Clear removes selected markers and native selection. Focus returns to native controls. Record actual tab order; full accessibility is unverified. |
| W06 | B/Y, m/N, range10–12; compare all rows/tooltips/units/KPIs. Rapid A→B→A, then point selection. | Three rows/points per plot; drift0.001/0.0011/0.0012m, shear600000/590000/580000N, peak0.0012m. No retired packet. Context changes may reset zoom/legend; selection-only may not. Invalid range retains last good context. |
| W07 | Drag bottom results splitter; resize/maximize/restore, including narrow widths. Orbit/zoom 3D, then charts. | Child clips to chart bounds; no overpainting native controls, displaced hit targets, stale rectangle or ghosts. Record cramped text/clipping. 3D and chart entity coupling is absent. |
| W08 | Hide Charts visible, click/scroll vacated area; select a native story; show charts. | Hidden child draws nothing and steals no input. Current context/selection restored without ghosts/duplicates. |
| W09 | Select story, Destroy charts, operate native controls, Recreate charts. | Closed→Loading→Ready; both plots replay current state. Record visible freeze/error/focus loss. One recreation does not prove20-cycle cleanup. |
| W10 | Minimize/restore and Alt+Tab away/back, each plot previously focused. | No orphan/always-on-top child. Bounds, state and focus recover; record lost first input. Parent exit is not subprocess-cleanup proof. |
| W11 | Enable OS IME; transfer focus between chart and existing slider numeric editing if available. Try Escape/input. | No trapped composition/keys or displaced candidate UI. No dedicated text/IME fixture exists: mark missing coverage BLOCKED; chart keyboard success cannot certify IME. |
| W12 | Move visible window between monitors with different scaling; repeat resize/hover/W02/W03. | Child bounds/hit targets follow monitor DPI. Record actual scale pair; one-monitor static tests cannot pass this. |
| W13 | Open a native popup/tooltip overlapping the child chart rectangle. | Correct occlusion, interaction and dismissal. Dedicated overlapping-popup fixture is absent: BLOCKED until separately implemented or a reproducible existing control is identified. OS-window overlay alone does not pass this row. |

## Blank observation record

Use PASS / FAIL / PENDING / BLOCKED / NOT TESTED. A passing scale cannot pass another. Preserve initial PENDING until actually observed; retain failures even if a workaround succeeds.

| Check | 100% | 125% | 150% | Evidence / observer / failure |
|---|---|---|---|---|
| W01 hover / values | PENDING | PENDING | PENDING | |
| W02 chart→native→both markers | PENDING | PENDING | PENDING | |
| W03 native→markers with child focused | PENDING | PENDING | PENDING | |
| W04 zoom / pan / legend preservation | PENDING | PENDING | PENDING | |
| W05 keyboard / focus / Escape | PENDING | PENDING | PENDING | |
| W06 range / units / stale state | PENDING | PENDING | PENDING | |
| W07 split / resize / clipping | PENDING | PENDING | PENDING | |
| W08 hide / input exclusion / restore | PENDING | PENDING | PENDING | |
| W09 destroy / recreate | PENDING | PENDING | PENDING | |
| W10 minimize / restore / focus | PENDING | PENDING | PENDING | |
| W11 IME / native editing | PENDING | PENDING | PENDING | Dedicated fixture absent; qualify coverage. |
| W12 monitor DPI transition | PENDING | PENDING | PENDING | Actual pair/hardware required. |
| W13 overlapping native popup | BLOCKED | BLOCKED | BLOCKED | Dedicated fixture absent. |

For every observed row record date/time, observer, exact source/binary SHA, environment/scale, action, expected/observed story/units, outcome, recording filename/hash/time offset, reproducibility and workaround. Label owner observations **owner-reported** until independently reviewed. Do not invent missing video/hash/environment from older runs. Keep private paths local.

## Separate open requirements

- Dock/float/reparent/layout persistence: **not implemented**, not N/A. Composition failure warrants a separately reviewed host spike, not a silent architecture rewrite.
- Runtime request/network-denial capture: **NOT TESTED**. CSP/allowlist/navigation entries do not prove zero HTTP requests; later trace must identify owned process/profile and cover startup/input/recreation/denial.
- Profile isolation, WebView subprocess closure,20× lifecycle/native+child runtime memory: **NOT TESTED**. Incognito is not process isolation. This branch launches/kills no apps. A later runner must establish profile and PID/start-time ownership before touching children.
- Presented-frame/selection p50/p95/p99, synchronous recreate latency,1/4/12 charts, dense10k/100k points and equivalent Tauri: **NOT TESTED**. Callback count/subjective speed is not timing evidence.
- WB-01 #30's narrow contract is accepted; WB-02 #24 GPU loading can follow separately. This matrix still uses independent small geometry/signal fixtures. Source-backed selection/calc/report parity remains downstream; #18 CRT/package **HOLD** remains.

## Reusable programmatic report gate

[assert-dashboard-smoke.ps1](../../scripts/assert-dashboard-smoke.ps1) validates a completed hidden run's exit code, bounded readable JSON, supplied interval, PASS/error, false manual/network/presentation claims, callback/refusal counts and three two-chart snapshots including identities/order/values. Missing/empty/malformed/stale reports fail even when the app logs a write failure and exits0.

It does **not** launch a smoke, authenticate a process, validate source/executable, isolate profiles or measure startup/cleanup. The owning launcher must refuse an existing report path **before** launch, retain actual timestamps/exit code and source/build/artifact receipts, then call:

```powershell
# Variables must come from the owning launcher, not fabricated timestamps.
./scripts/assert-dashboard-smoke.ps1 -ReportPath $freshReportPath `
    -ProcessExitCode $ownedProcess.ExitCode `
    -RunStartedUtc $actualStartUtc -RunFinishedUtc $actualFinishUtc
```

The result includes report SHA256 and narrow scope. Filesystem timestamps are consistency checks only: a copied/touched old report can satisfy them and is **not fresh execution proof**. Keep launcher/source/artifact evidence separately. No safe isolated launcher is claimed by this branch.

Offline controls (PowerShell7, fresh scratch path, no app launch):

```powershell
./scripts/test-dashboard-smoke-gate.ps1 -ScratchDirectory ./target/unique-gate-test
```

The positive control copies a public synthetic report; negative controls mutate/remove it. These test the gate and are **not new Windows smoke results**. No compiled inputs change; accepted #31 test/build results remain at their historical source heads rather than being relabeled as script validation.

### Recorded gate verification

At source `c8ab02ca23b86f456938172a1f161c83f9be63c2`, PowerShell7.6.5 on Windows passed **32 offline controls**: one accepted synthetic copy and31 rejected inputs (missing/malformed/empty/oversized/stale reports, failed exit, invalid time intervals, explicit failure/error, overclaimed manual/network/presentation, wrong context/counts/values/types). Both scripts parse successfully and Git whitespace checks pass. [Sanitized results and script/fixture identities](evidence/dashboard/windows-matrix-gate-controls.json) include every case. No app launch, new GPU/input test, runtime measurement or full Cargo gate occurred in this verification-only branch. The evidence follow-up changes docs/JSON only.
