# APP-01A.2: the visual system on the real viewport (2026-10-28)

**Status: draft, experimental `egui_test` only.** This PR is stacked on APP-01B
(#53, `feat/app-01b-viewport` @ `0ed117f`), which is itself stacked on APP-01A
(#50). Neither base is merged. It applies the #52 design target to the real
TALOS binary and the real GPU viewport. EtabExtension #467 stays HOLD. Tracks
[#49](https://github.com/tadoEng/egui_test/issues/49).

| Item | Value |
|---|---|
| Base | `feat/app-01b-viewport` @ `0ed117f` (#53, unmerged) |
| Backend pin | EtabExtension `9eb1c8792a1d41f3f81c60428afe89f1655782ba` (unchanged) |
| Dependencies | `talos-native` now lists `image` (0.25.10, PNG only), which was already in `Cargo.lock` through eframe. No new external crate, no version change |
| Assets | Inter variable (SIL OFL 1.1) and 20 Lucide icons (ISC). Sources, licences and hashes are in `apps/talos-native/assets/*/SOURCE.txt` |
| Toolchain | Rust 1.99.1, egui/eframe/wgpu 0.37 as published, no `[patch]` |

## What changed

### Type and colour

Inter is registered at three weights from one variable font: 400 body, 500
controls, 600 titles.

The type scale is:

| Use | Size |
|---|---|
| Body | 14 |
| Small | 12 |
| Section labels | 11, uppercase |
| Titles | 15 |
| Entity names | 18 |

Engineering values and warnings never go below 12 pt.

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

### Model: Docked or Floating, never over picking

Docked and Floating are a presentation toggle in the view toolbar, not a
permanent mode. Both show the same Explorer and Inspector and keep the
selection.

- **Docked (default).** Side panes, so geometry is never covered. The panes cap
  at a share of the window. At 900 pt the canvas keeps about 390 pt, including
  after shrinking from a wider window.
- **Floating.** Cards over a full-width canvas. A card, the toolbar, the
  version strip, the fidelity chip and the gizmo each block clicks and drags
  under them, so nothing picks or orbits through an overlay. The canvas still
  picks everywhere else.

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
| Never place the inspector over critical geometry or picking | Docked is the default. Floating cards and every overlay block picks under them (tested), and the canvas still picks outside them (tested) |
| 48 pt header target; validate at 900 pt and 100/125/150 % | One-row header (tested). 900 pt overflow, no overlaps, Ctrl+3 (tested). Real-binary captures at 1×, 1.25× and 1.5×, including 900 × 700 |
| No microtext for engineering values | 12 pt floor for values and warnings |
| Timeline: no invented data; synced with the header; no second controller | The strip sends the header's `SwitchVersion`; both reflect `session.requested()` (tested); the "no commit history" note is shown or named |
| Renderer beautification is separate | `lab-viewport` is untouched in this PR |
| Real Windows screenshots before approving a polish PR | **Still required.** Everything here is Linux, Xvfb and lavapipe |

## Tests

There are 35 `talos-native` tests. The UI tests run the real `App` and
`NativeModelView` through egui's AccessKit tree. Seven are new or rewritten
for A.2:

- panel modes keep the selection; overlays never pick through; the canvas still
  picks outside them;
- the version strip and the header picker act as one controller;
- the explorer text filter and type filter;
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
  XTest clicks, at 1, 1.25 and 1.5. It captures Model docked, a real pick, the
  same pick floating, Results, then the window resized to 900 × 700 (floating,
  then docked).
- `*.png`: those 18 frames, all public synthetic tower data.
- `gate.sh.txt` and `gate-a2-<sha>.txt`: the clean-worktree gate.

Not verified here: Windows rendering and fonts at real DPI, Narrator, and
keyboard focus under a window manager (Xvfb has none, so the captures click
tabs instead of using shortcuts). The owner's Windows check stays required.
