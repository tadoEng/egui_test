# APP-01C: ECharts / WebView2 in TALOS Results (2026-10-28)

**Status: draft, experimental `egui_test` only.** This branch is from `main` at
`de080d0` (#55 merged). EtabExtension #467 stays HOLD. Tracks
[#49](https://github.com/tadoEng/egui_test/issues/49).

The native Results foundation (APP-01C, #55) reads cached results through
`lab-results` and draws a story chart and table. This slice puts **ECharts in a
`WebView2` child window** over the chart's plot area on Windows. It is fed the
same validated profile through a typed, versioned contract. No egui chart work
was added: the native chart stays, unchanged, as the fallback (other platforms,
the in-process tests, or no `WebView2` runtime).

> **Not built or run on Windows by the author.** The Windows-only module
> (`webview_chart.rs`) cannot be compiled in this Linux container: no Windows
> Rust target is installed, and installing one is not an authorized tool
> install here. Everything platform-independent is tested here (below). The
> Windows build and the acceptance checklist at the end are the owner's.

## Shape

```text
State ──shown_profile()──► Results page ── Embed { plot area, Content } ──► App
                             (header, warnings,                               │
                              legend, table: egui)          hidden if: not Results, nothing
                                                            plotted, an egui popup open,
                                                            minimized, or chart unavailable
                                                                              ▼
                     chart_bridge::Bridge (platform-independent, tested)
                       Frame {protocol 1, version, source fingerprint, generation,
                              selected_story, plot (stored values), style (theme)}
                       accept(message) ──► Action::SelectResultStory / workspace keys
                                                                              │
          Windows: webview_chart::WebViewChart ── examples/support/webview_host.rs
                       (bounded offline Wry child; ECharts pinned offline asset)
                       evaluate_script("window.talosChart.apply(frame)")   ▲ ipc (≤ 1 KiB)
                                                                              │
                     apps/talos-native/assets/results_chart.html (ECharts page)
```

| Piece | File | Platforms |
|---|---|---|
| Contract: frames, messages, generations, refusals | `apps/talos-native/src/chart_bridge.rs` | all; unit-tested |
| Page: reserves the plot area, returns `Embed` | `apps/talos-native/src/pages/results.rs` | all |
| Shell: placement and hide rules, routing actions | `apps/talos-native/src/app.rs` (`EmbeddedChart` trait object) | all; UI-tested with a stand-in chart |
| `WebView2` host | `apps/talos-native/src/webview_chart.rs`, reusing `examples/support/webview_host.rs` through `#[path]` | **Windows only, not compiled here** |
| ECharts page | `apps/talos-native/assets/results_chart.html` + `assets/vendor/echarts/echarts.min.js` (pinned, offline) | Chromium-tested headless |

**New dependencies:** `serde` and `serde_json` for `talos-native`. Both are
already in the workspace. On Windows only, `wry =0.55.1` and `base64 0.22.1`,
the versions the Windows examples already pin. `Cargo.lock` gains three
dependency lines and no new package.

## Contract (`chart_bridge`)

- **Frame (host → page).** It carries:
  - `protocol: 1`;
  - `version` (`branch/version`) and `source` (the cached results' SHA-256
    fingerprint);
  - `generation`;
  - `selected_story`;
  - `plot`, the stored profile exactly as `lab-results` returns it (gaps as
    `null`, signs kept, the stored reference);
  - `style`, the TALOS theme colours, so the chart matches its card in light
    and dark.

  No path, file or anything to recalculate from is sent.
- **Generations.**
  - A new version, source, profile (metric, direction, group, case or story
    range) or style starts a new generation.
  - A selection change alone keeps the generation and is re-sent.
  - Showing nothing ends the frame, and the next one gets a new generation even
    if its content is the same. Generations are never reused.
- **Messages (page → host)**, JSON of at most 1 KiB. Unknown fields are
  refused (`deny_unknown_fields`).
  - `select_story` / `clear_selection`: accepted only if version, source and
    generation match the frame shown now and the story is in that frame.
    Anything else is refused as `Stale`, `UnknownStory` or `Malformed`, and
    counted.
  - `shortcut`: only Ctrl+1…5 and Ctrl+PageUp/PageDown. While the chart window
    has keyboard focus egui does not see the keys, so the page forwards these
    workspace shortcuts and nothing else.
  - `ready`: exact. It is consumed by the transport.
- **The state still decides.** An accepted selection becomes
  `Action::SelectResultStory`, which the reducer checks again against the
  stored profile.

## Placement and coexistence with the wgpu viewport

- The child window is placed over the plot area of the chart card only. The
  card's title, identity line, story-set warnings and legend stay egui, and so
  does the story table.
- It is **hidden** when:
  - the workspace is not Results (so it is never over the Model's wgpu
    viewport);
  - nothing is plotted (no cached results, a story-range mismatch, loading);
  - **any egui popup is open** (a selector or version list would otherwise be
    drawn under it);
  - the window is minimized;
  - the page is not ready yet, so an empty page never flashes.

  The frame is kept while a popup hides the window, so it reappears without a
  re-send.
- Placement uses physical pixels from egui points × `pixels_per_point`
  (`PhysicalBounds`, unit-tested at 100/125/150 % in the Windows examples).
  Bounds are re-sent only when they change. The page resizes ECharts with a
  `ResizeObserver`.
- **Known limit:** egui tooltips are not popups, so a tooltip next to the plot
  area can be drawn under the chart window.
- If the window cannot be created (no `WebView2` runtime) or placed, the
  native chart is used and a warning is logged.

## Security (inherited from the bounded transport)

- The page is authored HTML only, with no navigation: only the bootstrap
  `data:` URL and `about:blank` are allowed, and a blocked attempt is logged
  once.
- No new windows, downloads or drag-drop. Incognito; devtools, autofill,
  clipboard and zoom hotkeys are off.
- The page CSP is `default-src 'none'` with inline script and style only.
- The chart renders tooltips as canvas rich text, never HTML, so story names
  from client models are not interpreted as markup.

## Tests

| What | Where | Result here |
|---|---|---|
| Contract: frame identity and stored values, generations, re-send on a selection, failed-delivery retry, stale version/source/generation refused, unknown story, malformed/oversized/unknown messages, exact `ready`, shortcut allowlist | `chart_bridge_tests.rs` (8 tests) | **pass** |
| The page's **own** posted messages accepted for their frame and refused once stale | `chart_bridge_tests.rs`, reading `evidence/app-01c-echarts/messages.json` | **pass** (one of the 8) |
| Shell: chart placed over the plot area (beside the table) only on Results; hidden under an open selector list and back after; hidden on Model; frame carries the loaded version and the snapshot fingerprint; a page click selects; a late click after a metric change is refused; forwarded Ctrl+1 switches to Model; an unavailable chart leaves the native chart working | `ui_tests.rs` (3 tests, stand-in chart window) | **pass**; they caught a missing repaint after an accepted chart message (fixed) |
| ECharts page in headless **Chromium** (the engine under `WebView2`): ready first; a row click posts `select_story` with identity; clicking the selected story clears; clicks outside the grid post nothing; Escape clears; only workspace shortcuts are forwarded; an older generation is ignored; malformed frames are refused; `fetch` blocked by the CSP; no page errors | `evidence/app-01c-echarts/chart_page_test.js` → `chart_page_test.txt` | **11/11 pass** |

Receipt: [`evidence/app-01c-echarts/linux-receipt.txt`](evidence/app-01c-echarts/linux-receipt.txt) (fmt, Clippy on the TALOS crates, 152 workspace tests).

The light and dark images in `evidence/app-01c-echarts/` are the page with an
**authored synthetic** frame (made-up values, a gap, a stored reference), not
the owner's data.

## Windows acceptance (owner)

Build and run from this branch's head:

```text
cargo build -p talos-native --locked
cargo test  -p talos-native --locked
cargo run   -p talos-native -- --public-results-demo
cargo run   -p talos-native -- --version <folder>   (private; do not publish captures)
```

1. **Build.** `talos-native` compiles and Clippy is clean on Windows
   (`webview_chart.rs` and the included `webview_host.rs` tests run as part of
   `cargo test -p talos-native`).
2. **Chart appears.** On Results (Ctrl+2) the ECharts chart fills the chart
   card's plot area below the egui title/legend, matching the theme (check
   light and dark). With no `WebView2` runtime, the native chart is shown
   instead and a warning is logged.
3. **Both selection directions.**
   - Clicking a story row in the chart highlights it in the story table.
   - Clicking a table row highlights the band in the chart.
   - Clicking the selected story clears it; Escape in the chart clears it.
4. **Stale denial.** Change metric/direction/case quickly while clicking the
   chart. No selection lands on the wrong profile. With `RUST_LOG=debug`,
   refusals are logged as `chart message refused (Stale…)`.
5. **Z-order.** Open the Metric, Direction, Group, Case, story-range and
   Version lists. The chart hides while each list is open and returns when it
   closes. No list is ever drawn under the chart.
6. **wgpu viewport coexistence.**
   - Switch Model ↔ Results repeatedly (tabs and Ctrl+1/Ctrl+2). The chart
     never shows over the Model viewport.
   - The viewport keeps rendering and picking normally (orbit, click,
     Ctrl-click) with no flicker or black frames.
   - Switch versions on each page.
7. **Focus.**
   - After clicking the chart, Ctrl+1…5 and Ctrl+PageUp/PageDown still switch
     workspaces.
   - Clicking back into egui returns keyboard input to egui (E/I/F on Model).
   - Typing in an egui field is unaffected.
8. **Resize and DPI.**
   - Resize from full width down to 900 × 560, and move between monitors at
     100/125/150 %. The chart tracks the plot area exactly, with no offset or
     overhang onto the table, the header or the window edge.
   - Minimize and restore.
9. **Real folder.** `--version <folder>` on the owner's data: all five
   responses plot in both directions, with the same values as the story table.
   Keep any capture private.
10. **Offline and shutdown.** No network traffic, and no `msedgewebview2`
    process left after exit.

The record should be one line per item (pass/fail and a note), with no client
data in public.
