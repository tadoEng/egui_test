# EtabExtension chart presentation reuse

This experimental cached-result viewer adapts the production desktop **presentation** from EtabExtension at the same immutable source pin `9eb1c8792a1d41f3f81c60428afe89f1655782ba`. The source files were checked against their committed Git blobs; unrelated local checkout changes were not used or modified.

| Source | Reused presentation | Git blob |
| --- | --- | --- |
| [AnalysisChartPanel.tsx](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/apps/desktop/src/components/analytics/charts/AnalysisChartPanel.tsx) | Bordered rounded panel, external heading, muted caption, outlined badge | `9c8bf58fa5600575fb5f760c21c567a0afb2513e` |
| [chartSpec.ts](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/apps/desktop/src/components/analytics/charts/chartSpec.ts) | Top legend, swapped axes, middle axis names, twoUp grid proportions, unsmoothed lines | `4f4e20753ac91208b2008fa31f64ec9cedbbd48f` |
| [calcOptions.ts](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/apps/desktop/src/components/analytics/charts/calcOptions.ts) | REPORT_PALETTE categorical colours, red dashed legend-visible reference series | `2236d519f98657b8f3d48cec9d7ee268c96e8884` |
| [index.css](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/apps/desktop/src/index.css) | Default light background, border, foreground and muted-text appearance | `b620e794069a1f5b7b4eee7c0434e34a2bbe1ca9` |
| [Chart standards](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/docs/chart-standards.md) | Unit-bearing value axes, named limits, verbatim category identity, chart-only scope | `7350cd74d757b8b198567d550f4822d4351cb7c7` |

Apache-2.0 source attribution/license is retained in `assets/vendor/etabextension/`. No production EtabExtension file is changed.

## Intentional adaptations

- Use standalone HTML/CSS panel equivalents and ECharts' default light presentation; the native egui shell retains its existing theme. This is not a new React/Tauri runtime or automatic theme parity implementation.
- Preserve all existing Rust packets, numeric fields, units, raw signed positive/negative series, nulls and source story order. Keep `inverse:true` so the original roof-first order stays visually roof-first. **Do not copy** the production builders' demand aggregation, absolute/max selection, rounding, missing-to-zero fallback or reversed/filtered categories.
- The cached allowable ratio becomes a named red dashed constant-value line series, matching the production visual convention. It repeats the existing stored scalar over the existing categories; no threshold is calculated. It is labelled **Stored allowable ratio**, with no pass/fail/governing claim.
- Retain small clickable point symbols for the existing story-selection interaction. Production chartSpec normally hides line symbols. Keep the selected-story marker distinct from the stored reference; source/generation IPC remains unchanged.
- Keep the existing vertical two-chart layout and zoom controls. Apply minimum pixel insets to the production twoUp proportions so Story/quantity names, long labels and slider controls fit narrow charts. Headings truncate only visually and retain full text in the DOM/title tooltip. The chart tooltip keeps the raw values and category identity.
- Persistent cached/unverified freshness text remains outside both charts. No source-model analysis freshness, ext-render numerical/chart-export parity or engineering result approval is established by matching style.

## Validation

```powershell
cd '<style-worktree>'
npm --prefix tests/chart-layout ci --ignore-scripts
node tests/chart-layout/cached.cjs
node tests/chart-layout/style.cjs
```

The existing public native fixture packet can also be consumed with `cached.cjs --public-probe='<owned-public-fixture-report>'`. It accepts only the independently authored fixture identity, never arbitrary user result files.

`style.cjs` uses LAB-CACHED-STYLE-01-r1 authored signals only: nine resize scenarios at three headless browser pixel ratios. It checks external titles, palette, named dashed stored reference, axis labels, unchanged order/null/sign/value arrays, a headless point-selection message, and selection-only preservation of zoom/legend/reference. Browser contexts/server are closed and owned root exit is observed before PASS reporting. A browser pixel ratio is not physical Windows DPI or native WebView2 input/compositor evidence.

The [committed evidence receipt](evidence/cached-chart-style/validation.json) records compiled source0dad38c: full offline82 CPU executions (13 shared duplicates),2 GPU instances ignored, fmt/native all-target/all-feature/strict Clippy/docs0/WASM PASS. The styled Debug executable is25,237,504bytes, SHA256 ad74b9cb42d03d66a11cb18b955c220042cc9ccc39674847d984996f5071107f. An actual public fixture invocation of that binary passed the styled HTML regression; owned input/report directories and browser root were released. No GUI was launched by the agent. The final evidence-only commit changes no compiled input. #35's prior Release run stopped at its unchanged2GiB available-memory guard; no style Release/packaged acceptance is claimed. Physical Windows input/focus/DPI, browser-group cleanup, engineering parity and all migration gates remain open. No private data, derived plot, screenshot, path, count or timing enters public evidence.
