# APP-01C: cached engineering results in the Results workspace (2026-10-28)

**Status: draft, experimental `egui_test` only.** This branch is from `main` at
`816fa22` (#50, #53 and #54 merged). EtabExtension #467 stays HOLD. Tracks
[#49](https://github.com/tadoEng/egui_test/issues/49).

This is the **native** half of APP-01C: real cached results, read through the
pinned EtabExtension types, shown as a story chart and a story table. The
**ECharts / WebView2** half is a separate, Windows-only step (see "Not in this
slice"). It is not replaced by the native chart.

| Item | Value |
|---|---|
| Base | `main` @ `816fa22` |
| Backend pin | EtabExtension `9eb1c8792a1d41f3f81c60428afe89f1655782ba` (unchanged) |
| New crate | `crates/lab-results`: the cached-results reader, moved from `examples/native_results` (the Windows examples include the same files through `#[path]`), plus stored story-response profiles |
| Dependencies | `talos-native` now depends on `lab-results`, so it builds `ext-calc` (and its Polars closure), as `native_results` already did. Test-only: `serde_json`, `tempfile` (already in `Cargo.lock`). No new external crate, no version change |
| Toolchain | Rust 1.99.1; egui/eframe/wgpu 0.37 as published |

## What it does

```text
version folder (read only)             lab-results (pinned ext-calc / ext-core types)
  manifest.json ─────────────────────► VersionManifest   ─┐
  model.e2k ──► lab-e2k-mesh (Model)                       ├─ Snapshot (identity, units checked)
  results/calc_output.json ──────────► CalcOutput         ─┤      │
  results/source-units.json ─────────► SourceUnitMetadata ─┘      ▼
                                                      Response profile: stored values per story
                                                      (gaps kept, signs kept, nothing recalculated)
                                                               │
                         TALOS Results ◄── ResultsQuery (metric, direction, group, case, stories)
                         chart + story table, selection per version
```

- **Sources.**
  - `talos-native` still opens the public tower, which has no cached results.
  - `--public-results-demo` opens the authored LAB-REAL01 fixture: a 2-story
    E2K with authored cached values. It is labelled **Authored fixture** and is
    not an analysis.
  - `--version <folder>` (repeatable) opens real version folders in place,
    read-only. Identity comes from each folder's `manifest.json`. Only
    `manifest.json`, `model.e2k`, `results/calc_output.json` and
    `results/source-units.json` are read; nothing is written or scanned.
- **Identity.**
  - Results load on the loader thread with the model, under the same ticket.
    A stale or mismatched load is refused exactly as in APP-01A.
  - Cached results are shown only for the version their manifest names.
  - `Snapshot` checks that the results' branch/version match the manifest and
    that the units are consistent.
  - The page always says **Cached · freshness not verified**: matching names do
    not prove the results were computed from this exact geometry.
- **Profiles (stored fields only):**
  - wind drift and seismic drift: stored positive and negative envelopes, plus
    the stored allowable ratio;
  - wind displacement: envelopes, plus stored story limits when present;
  - story shear: stored `Vx` for X-direction profiles, `Vy` for Y;
  - **story moment: both stored `Mx` and `My`**.
- **Only stored responses** are offered in the Metric list, and groups and cases
  come from the results. A case is offered only if it has rows for the chosen
  group, so every pair offered plots.
- **Chart:**
  - stories top to bottom in stored order, values along x;
  - a gap breaks the line;
  - the stored reference is drawn dashed;
  - hover shows the story's stored values; a click highlights the story.
- **Story table:** one row per story with every series. Clicking a row
  highlights it in the chart; clicking again clears it. Each row's state is in
  the accessibility tree.
- **Mismatches are visible:**
  - The chart flags cached stories that are not in the version's model.
  - A story range the cached profile does not name shows **"Story range not in
    these cached results"**, with an explicit "Show every cached story" choice,
    instead of widening silently.
- **Selection scope:** a highlighted result story clears as soon as another
  version is requested and never touches the Model selection.

## Review of `0919990` (#49): what changed

| Finding | Resolution |
|---|---|
| P1: overturning-moment axis | No overturning claim and no assumed pairing. The metric is **"Story moment · stored Mx, My"**, and both stored components are plotted and tabulated with their field names, for the X- or Y-direction profile list. Pinned `ext-calc` envelope rows pair `max_vx` with `max_my`; whether that is the overturning convention for a direction is not verified here, so no component is called the overturning moment. Tests cover both directions, signs and units. |
| P1: story range silently widened | `State::shown_profile` returns `Shown::RangeMismatch(missing)` when either end is absent from the cached profile. A regression test covers a model story (L1) that the cached profile lacks. |
| P2: group × case combinations | `response_choices(…, group)` offers only cases with rows for that group. The state re-picks a case (with a notice) only when the current one has no rows in the new group. |
| P2: story set vs E2K | Flagged in the chart subtitle; the freshness badge is always shown. |
| P2: selection on version switch, leak into Model | Cleared on the switch request; a test checks the Model selection and story stay untouched. |
| Follow-up at `c4c0340`: stale "overturning" wording in the state docs | Reworded: "the stored story moment components Mx and My (no overturning axis is inferred)". |
| Follow-up at `c4c0340`: highlighted story vs a new story range | A highlighted story that the new range no longer shows is cleared (test: `a_story_range_that_excludes_the_highlighted_story_clears_it`). |
| Follow-up at `c4c0340`: model stories missing from the cache | The chart also warns "N model stories have no cached row". A missing base level is common; a missing intermediate level may mean the results come from another model. |
| P2: Windows example smoke after the move | **Still required on Windows.** The examples include the moved files through `#[path]` and compile only on Windows, which cannot be checked from this container. |

## Tests

- `lab-results` (Linux): the 5 reader tests that ran only on Windows before,
  plus 6 response tests:
  - only stored responses offered;
  - drift order, signs and reference;
  - named shear fields, and moment components in both directions;
  - gaps vs unknown stories;
  - moment values and signs;
  - group-filtered cases.

  One opt-in local test, `LAB_RESULTS_VERSION=<folder>`, prints only which
  responses load, never values or names.
- `talos-native`: state tests on real temporary version folders:
  - the fixture's stored choices, and refusal of anything not stored;
  - the story-range mismatch;
  - stored moments;
  - the per-version result-story selection, kept apart from the Model;
  - absent and refused results.

  UI tests:
  - the chart title, stored table values and the freshness badge;
  - the table row ↔ chart selection, with toggled state in AccessKit;
  - only stored metrics in the list;
  - moments without an overturning label;
  - the tower still showing no cached results.

## Local check with the owner's data (not published)

The owner's version folder (manifest, model, `results/calc_output.json`,
`results/source-units.json`) was read locally in a private scratch folder and
was never committed. It passes every `Snapshot` check: identity, unit
provenance, story order and membership, finite values. All five response types
load in both directions for every stored case. No name, value, count or
screenshot from it appears in this repository.

## Not in this slice

- **ECharts / WebView2 in Results** (Windows-only Wry child). It needs typed
  version/fingerprint/generation messages, stale-event denial,
  focus/z-order/resize, and coexistence with the wgpu viewport. That code
  compiles only on Windows; it comes next as a separate, explicitly flagged
  step, and is not claimed tested here.
- Verifying the overturning-moment convention against an authoritative
  producer or ETABS output.
- `ext-api` repository routing (APP-01D), recalculation, reports and any write
  path.
