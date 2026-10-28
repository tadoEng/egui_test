# APP-01A.2: the visual system on the real viewport (2026-10-28)

**Status: draft, experimental `egui_test` only.** This PR is stacked on APP-01B
(#53, `feat/app-01b-viewport` @ `664d04e`), which is itself stacked on APP-01A
(#50). Neither base is merged. It applies the #52 design target to the real
TALOS binary and the real GPU viewport. EtabExtension #467 stays HOLD. Tracks
[#49](https://github.com/tadoEng/egui_test/issues/49).

| Item | Value |
|---|---|
| Base | `feat/app-01b-viewport` @ `664d04e` (#53, unmerged; merged into this branch after the #53 story-selection fix) |
| Backend pin | EtabExtension `9eb1c8792a1d41f3f81c60428afe89f1655782ba` (unchanged) |
| Dependencies | `talos-native` now lists `image` (0.25.10, PNG only), which was already in `Cargo.lock` through eframe. No new external crate, no version change |
| Assets | Inter variable (SIL OFL 1.1) and 20 Lucide icons (ISC). Sources, licences and hashes are in `apps/talos-native/assets/*/SOURCE.txt` |
| Toolchain | Rust 1.99.1, egui/eframe/wgpu 0.37 as published, no `[patch]` |

## What changed

### Type and colour

Inter is registered at three weights from one variable font: 400 body, 500
controls, 600 titles.

The type scale is 12/13/15/18:

| Use | Size |
|---|---|
| Small and section labels (uppercase) | 12 |
| Body | 13 |
| Titles | 15 |
| Entity names | 18 |

Nothing (engineering values, warnings, labels) goes below 12 pt.

Surfaces are calm, with one accent colour:
- Results selectors and cards have no borders.
- Inspector rows are separated by space, not rules.
- The only remaining rule is the docked panes' edge.

Dark and light tokens follow the system theme. The canvas keeps the
renderer's own background. State is never shown by colour alone: every dot has
a word, and in compact forms the word moves to the tooltip and accessible name.

### Header: one 48 pt row

The row holds:
- brand, project, branch;
- the version picker and the Synthetic pill;
- the workspace tabs, with icons;
- Read-only and ETABS status, right-aligned.

The separate tab bar and the bottom status bar are gone, and the warning band
appears only on failure. Below 1100 pt:
- the project name and the status words collapse to icons, which keep their
  accessible names;
- Compare, History and Reports move into **More**, except the active workspace;
- Ctrl+1…5 still reach every workspace.

### Model: Floating or Docked, never picking through

Floating and Docked are a presentation toggle in the view toolbar, not a
permanent mode. Both show the same Explorer and Inspector and keep the
selection.

- **Floating (default).** Explorer, Inspector and the view tools are cards with
  margins, rounded corners and a soft shadow. The canvas runs edge to edge
  underneath. A card, the toolbar, the version strip, the fidelity chip and the
  gizmo each block clicks and drags under them, so nothing picks or orbits
  through an overlay. The canvas still picks everywhere else.
- **Docked.** Side panes, so geometry is never covered. The panes cap at a
  share of the window. At 900 pt the canvas keeps about 390 pt, including after
  shrinking from a wider window.

**Compact windows (below 1100 pt, owner decision on #54).** One contextual
panel at a time, in both modes:
- the Explorer by default;
- a member pick makes the Inspector the context, replacing the Explorer;
- clearing the selection, a story change or a version switch hands the context
  back to the Explorer.

The visible panel's header button switches to the other panel: **Switch to
explorer (I)** / **Switch to inspector (E)**. The hidden one keeps a corner
button (**Show explorer (E)** / **Show inspector (I)**). E and I do the same
from the keyboard, and an explicit toggle always works.

The user's panel intent (`explorer_open` / `inspector_open`) is kept apart from
what the width can show (`ModelState::shows_explorer` / `shows_inspector`).
Resizing 1280 → 900 → 1280 restores both panels, keeps the selection, project
and version, and loads nothing. Fit is unchanged: per the owner, a
viewport-aware Fit is a separate camera-and-picking task.

### Canvas overlays

- **Gizmo:** built from the real camera basis.
- **View toolbar:** Fit, clear story highlight, Docked/Floating, and a
  navigation hint.
- **Fidelity chip:** backend tier counts, top-left of the free canvas. Below
  540 pt it shows numbers only.
- **Version strip.**

### Version strip: one controller

The version strip is one node per version on the branch. Clicking a node sends
the same `SwitchVersion` as the header picker, and both always show the same
current version. It invents nothing: no commit history, no dates, no analysis
runs. It says so in the strip, or behind an info icon with that text as its
accessible name when the strip is narrow. The node summaries are the fixture's
own descriptions.

### Explorer

- A name/section filter.
- Story rows that end on a whole row.
- Type rows coloured with the colour each class is actually drawn in; a type
  row filters, never selects.
- The virtualised entity list.
- The story filter and the selection are kept distinct (owner's #53 Windows
  note). When the selection lies outside the filtered story, the Explorer says
  so, for example "Selected C0_0 is on L1, outside the L5 filter.", and offers
  **Show in Explorer**. That moves the filter to the selection's story, or
  clears it if the selection spans stories, and keeps the selection.
- A source-units footer.

### Inspector

- An entity title.
- Tier and display chips that wrap as a unit.
- Amber notes for withheld, approximate and ambiguous states.
- Grouped Section/Display/Source rows.
- **Copy source key.**

Backend reason names read as words, for example "Resolved · local axis rotation
unavailable". The mapping is mechanical: it splits the enum name and never
re-labels a tier.

### Results

Each selector's label comes before its control: Metric, Direction, Case,
Stories From → to.

The chart card draws only the real story axes at real elevations. It says there
are no analyzed results for this version and shows the resolved query. No
values are plotted or invented.

## Owner rules from the #52 review

| Rule | Where it holds |
|---|---|
| Docked vs floating is not a permanent mode | Toolbar toggle; `PanelMode` is presentation-only; the test keeps the selection across both |
| Never place the inspector over critical geometry or picking | Floating cards and every overlay block picks under them (tested), and the canvas still picks outside them (tested). Docked never covers geometry. Below 1100 pt only one panel shows; the 900 pt unobscured model region is tested in the default and selected states |
| 48 pt header target; validate at 900 pt and 100/125/150 % | One-row header (tested). 900 pt overflow, no overlaps, Ctrl+3 (tested). Real-binary captures at 1×, 1.25× and 1.5×, including 900 × 700 |
| No microtext for engineering values | 12 pt floor for values and warnings |
| Timeline: no invented data; synced with the header; no second controller | The strip sends the header's `SwitchVersion`; both reflect `session.requested()` (tested); the "no commit history" note is shown or named |
| Renderer beautification is separate | `lab-viewport` is untouched in this PR. The design table's grid floor and gradient canvas are therefore **not** here; they belong in that separate renderer change |
| Real Windows screenshots before approving a polish PR | **Still required.** Everything here is Linux, Xvfb and lavapipe |

## Tests

There are 41 `talos-native` tests, including the two story-selection tests merged in from #53 (`0e7330b`). The UI tests run the real `App` and
`NativeModelView` through egui's AccessKit tree. Seven are new or rewritten
for A.2:

- Floating is the default; panel modes keep the selection; overlays never pick
  through in either mode; the canvas still picks outside them;
- compact (state): one panel shown, a pick switches to the Inspector, E/I swap,
  intents survive widening, a closed Inspector stays closed, a version switch
  resets the context;
- compact (UI, 900 × 700): the unobscured model region is at least 560 × 450 pt
  with the default Explorer and with the Inspector after a pick. Also checked:
  the keyboard swap, 900 → 1280 → 900 with the same scope and pick, and Docked
  at 900;
- the version strip and the header picker act as one controller;
- the explorer text filter and type filter;
- the selection outside the story filter: the notice, and **Show in Explorer**
  keeping the selection (state and UI);
- at 900 pt: the More overflow, no header overlaps, shortcuts still route, and
  the canvas keeps at least 380 pt;
- shrinking from 1280 to 900 pt narrows the docked panes. This test fails
  against the earlier caps;
- Results selector labels sit before their controls on the same line;
- the inspector text stays inside its pane (docked) and its card (floating).

The existing inspector width × scale × theme sweep still passes with the new
chips.

## Evidence (`docs/native/evidence/app-01a2/`)

- `capture_talos.py`: the real binary under Xvfb + Mesa lavapipe with real
  XTest clicks, at 1, 1.25 and 1.5. It captures Model floating (the default), a
  real pick, the same pick docked, Results, then the window resized to
  900 × 700 with one contextual panel (the Inspector for the pick, then the
  Explorer, floating and docked).
- `*.png`: those 21 frames, all public synthetic tower data.
- `gate.sh.txt` and `gate-a2-d7fab97.txt`: the clean-worktree gate at
  `d7fab97`. Every step is green except the known G-ENV-1 whole-workspace Clippy
  exception (`webview_stage_a.rs:393` `eprintln!`). Results:
  - workspace tests: 159 passed, 0 failed, 7 ignored;
  - Mesa GPU, `talos-native` + `lab-viewport`: 39 passed;
  - GPU examples: 41 passed.

  The commit after it adds only this receipt.

Not verified here: Windows rendering and fonts at real DPI, Narrator, and
keyboard focus under a window manager (Xvfb has none, so the captures click
tabs instead of using shortcuts). The owner's Windows check stays required.
