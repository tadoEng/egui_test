# APP-01 visual design target (2026-10-28)

**Status: proposal for owner review.** A static mock, not the application. Nothing
here changes `talos-native`; implementation follows only after sign-off
(APP-01B first, then APP-01A.2). Tracks
[#49](https://github.com/tadoEng/egui_test/issues/49).

## What is real and what is proposed

| Element | Status |
|---|---|
| 3D image | **Real.** An unmodified crop of the existing native wgpu viewport (`examples/native_e2k`, the renderer APP-01B mounts) rendering the public synthetic tower `main/v2`. Captured headlessly with lavapipe (`capture-viewport.sh`; a re-capture differs from the committed crop by 0 pixels); only the example's header and two diagnostic text overlays are cropped away. |
| Explorer counts, Inspector values, fidelity chip | **Real** `lab-e2k-mesh` output for that fixture: 672 entities (240 columns, 372 beams, 12 floors, 48 walls); slab `F` at L12 in v2 = `S8`, `0.2286 m (0.75 ft)`, `TOP`, `FlatAtDeclaredFace`, `Approximate [ShellInsertionPointIgnored]`; 59 exact · 612 resolved · 1 approximate · 0 withheld. |
| Story names and elevations on the Results axis | **Real** (13 stories, Base 0 m to L12 48.463 m). |
| Results curve (`results-illustrative-*`) | **Invented**, with a banner and watermark. The honest default is `results-dark-1080p`: no analyzed result, nothing plotted. |
| Layout, type, colour, icons, view tools, axis gizmo, timeline, Explorer filter | **Proposed.** |

## Decisions applied (owner, 2026-10-28)

- **Font:** Inter (OFL, `assets/fonts/OFL-Inter.txt`), one family. Scale: 11 (section
  labels) · 12 (meta) · **13 (UI base)** · 15 (titles) · 18 (entity name), tabular
  numerals.
- **Icons:** a 24-icon Lucide subset (`lucide-static` 1.63.0, ISC,
  `assets/icons/LICENSE-lucide.txt`; tarball sha256 in `assets/icons/SOURCE.txt`).
  SVG files, no icon-font or crate. Icon-only tools carry labels (tooltips and
  accessible names).
- **Title bar:** native Windows decorations are kept. The mock starts below them.
- **One header row (experiment):** identity, workspaces and status share 48 px. At
  900 px the project name and the ETABS text collapse and Compare / History /
  Reports move into **More**. This replaces the old header, warning band, tab
  bar, Model toolbar, footer and status bar: six bands become one.
- **Overlay vs docked (prototype, both shown):** `model-*` floats the Explorer and
  Inspector as anchored cards; `model-*-docked` shows the same content as edge
  panes. Observations:
  - Overlay gives the model the most room at 1080p.
  - Docked is calmer and never covers geometry.
  - At 900 px both are tight. The overlay Inspector covers the right of the
    model; docked shrinks the canvas.
  - **Recommendation:** docked by default, overlay as an option. Decide after
    APP-01B on Windows.
- **Version timeline (signature):** a strip at the bottom of Model. It shows only
  `main/v1` and `main/v2`, each with its authored source difference, and states
  "Synthetic snapshots: no commit history, dates or analysis runs." The header
  picker stays the authority: clicking a timeline node issues the same
  `SwitchVersion` action. Compare will later select two nodes on the same
  strip. History is the full graph once `ext-api` provides one, so the strip
  stays a compact view of it, not a second history.
- **Truth labels designed in, not bolted on:**
  - fidelity as text plus colour (never colour alone);
  - "backend fidelity" separate from "native display";
  - converted lengths with their source value;
  - an Approximate callout that explains what is not drawn;
  - `Synthetic` on the version and on the Results scope.

## Renders (`renders/`)

| File | Viewport |
|---|---|
| `model-dark-1080p.png`, `model-light-1080p.png` | 1920 × 1080 @ 1.0 |
| `model-dark-125pct.png` / `model-dark-150pct.png` | 1536 × 864 @ 1.25 / 1280 × 720 @ 1.5 (1920 × 1080 physical) |
| `model-dark-900px.png` | 900 × 640 |
| `model-dark-1080p-docked.png`, `model-dark-900px-docked.png` | docked comparison |
| `results-dark-1080p.png`, `results-light-1080p.png`, `results-dark-900px.png` | honest empty state |
| `results-illustrative-dark-1080p.png` | invented curve, labelled |

**Found by the visual gate:** at 150 % (1280 logical px) the fidelity chip collided
with the view tools. Below 1500 px it now moves to the top edge of the canvas.
The light theme keeps the real viewport's dark background (`rgb(18, 25, 35)`),
because a light canvas needs a renderer background token, which belongs to
APP-01B.

## The renderer is part of the "wow" gap (proposals, not captured)

The real viewport is honest but visually flat. Proposed for APP-01B and
APP-01A.2, each as its own reviewable change with before/after captures:

1. Thin dark edge lines on frame and shell silhouettes.
2. A ground grid at Base with soft distance fade.
3. A subtle vertical background gradient, plus a light-theme canvas token.
4. A desaturated material palette, with colour reserved for selection, fidelity
   and results.
5. Selection highlight and hover outline.
6. Optional fidelity colouring mode (Exact / Resolved / Approximate) instead of
   class colouring.

None of these may change geometry, fidelity or picking.

## Rebuild

```bash
python3 -I build.py            # template.html + Lucide SVGs -> target.html
node capture.cjs .             # preinstalled Playwright Chromium -> renders/
```

`target.html#model,dark` (or `results`, `light`, `narrow`, `docked`,
`illustrative`) opens any variant in a browser.
