# APP-01A — TALOS native application shell (2026-10-28)

**Status: draft, experimental `egui_test` only.** This is a tab-first
application shell on public synthetic data. It is not a production migration,
not a six-gate acceptance and not a merge request; EtabExtension #467 stays
HOLD. Tracks [#49](https://github.com/tadoEng/egui_test/issues/49).

| Item | Value |
|---|---|
| Base | `main` @ `a81695baaad8622b2c8437abe6f08936feca8f4a` (merged #48) |
| Branch | `feat/app-01a-native-shell` (new; not the #48 branch) |
| Backend pin | EtabExtension `9eb1c8792a1d41f3f81c60428afe89f1655782ba` (unchanged) |
| Toolchain | Rust 1.99.1 (`rust-toolchain`), egui/eframe 0.37.0 as published, no `[patch]` |
| `Cargo.lock` | one new entry, the `talos-native` package itself; no new or changed dependency |
| Sources studied | storytold/artcraft `1f969a9224934a55417daf10359a6fb1458b63f4` (`artcraft-native/`); storytold/craft-libs `b4afa6d22ae41bd36524c32dbcab3ff257ae4cdd` (inventory only; the compatibility spike is a separate PR) |

## Run it

```powershell
cargo run --release --locked --offline -p talos-native
```

No ETABS, network or files are needed: the two demo versions are compiled into
the binary from the committed REAL-03 fixtures.

## What it is

- **Header** (one line): `TALOS · Public demo project › main › [v2 ▾]` plus
  one `Synthetic demo` tag whose tooltip says the versions are not
  EtabExtension commits and nothing has been analyzed. A warning band appears
  **only** when a load fails. Units, fidelity and connectivity are not in the
  header.
- **Workspace tabs**: Model, Results, Compare, History, Reports. Each tab owns
  its own page; Model and Results never render together. Compare, History and
  Reports are routed placeholders that say what they wait for and have no
  controls.
- **Model**: the view fills the page (an APP-01A placeholder that states
  APP-01B mounts the GPU viewport there). Explorer (E) and Inspector (I) are
  optional side panels. The footer holds this workspace's context: source units
  (`kip · ft · °F`), the explicit length conversion (`0.3048 m/ft`) and the
  backend-fidelity counts.
- **Results**: chart-first, no 3D. Metric, direction, case and story-range
  selectors edit a typed `ResultsQuery` for the header's version. The case
  selector is disabled ("No analyzed cases"), and the chart area says
  `No analyzed results for main/vN` with the resolved query. No value is shown
  or invented.
- **Status line** (compact): `Read-only · ETABS not connected`.

### The two synthetic versions

| Version | Source | Source-backed difference |
|---|---|---|
| `main/v1` | `real03-shells/fixtures/middle.e2k` | slab `S` `CARDINALPOINT "MIDDLE"` → `CentredMiddle`, body drawn ±t/2 |
| `main/v2` | `real03-shells/fixtures/top.e2k` | slab `S` `CARDINALPOINT "TOP"` → `FlatAtDeclaredFace`, `Approximate [ShellInsertionPointIgnored]` |

Both are independently authored public fixtures (sha256 in
`fixtures.sha256`). They are shown as **synthetic snapshots**, never as
committed EtabExtension versions. The source units are Kip/ft/°F. Lengths are
shown in metres because `lab-e2k-mesh` converts them at the declared factor,
and each converted value is shown with its source value, e.g.
`0.1524 m (0.5 ft)`. The only backend-crate change is a read-only
`Mesh::source_units()` accessor that reports the declared units and the
factor already applied.

## Ownership boundaries (what APP-01B/C/D plug into)

```text
            Action ──► State::apply ──► Ok(effects) | Err(Refusal)
              ▲              │
 pages / tabs / shortcuts    ├─ Session: project, requested version, Ticket{version, generation},
 (read &State, return        │           Document (immutable, Arc) ── scope = (Revision, generation)
  Vec<Action>)               ├─ Workspace (active tab)
              ▲              ├─ ModelState: explorer/inspector open, Selection (scoped)
 ModelView::show(ui,         └─ ResultsState: ResultsQuery{version, metric, direction, case, stories}
   &ViewFrame) -> ViewIntent
              ▲
 Loader runs Effect::Load off the UI thread and returns (Ticket, Result<Arc<Document>>)
```

- `State::apply` is the only transition. It performs no I/O and is tested
  without egui.
- **Document identity**: every load is issued with a `Ticket {version,
  generation}`. A response is applied only if its ticket is the current one;
  otherwise it is refused (`StaleResponse`) and counted. A response whose
  document is for a different version than its ticket fails the load closed
  (`MismatchedResponse`). While a version loads, no page shows the previous
  version's content.
- **Selection** is owned by the shell (`lab_e2k_mesh::pick::Selection`, the
  same type the native viewport uses), scoped by `(Revision, generation)`. A
  version switch clears it, and a pick made in an old scope is refused
  (`StaleSelection`), including when identical bytes are reloaded as another
  version.
- **Views** (`view::ModelView`) get a borrowed `ViewFrame {document, scope,
  selected, picking_allowed}` for one frame and return `ViewIntent::Pick
  {scope, ..}`. In APP-01B the existing `NativeViewport` keeps its camera, GPU
  buffers (keyed by geometry revision) and CPU picking, and turns its click
  into an intent instead of mutating its own selection. The shell does not
  change. Explorer picks use the same action.
- **Workspace state** persists across tab switches within a session (explorer
  and inspector visibility, selection, results query). Version-scoped parts
  are revalidated on every load: the selection expires, and a story range that
  does not exist in the new version is reset with a visible notice.
  Persistence across launches is not implemented.

## Source inventory: ADOPT / ADAPT / DEFER

ArtCraft Native (`artcraft-native/`, MIT OR Apache-2.0; the ArtCraft name,
marks, fonts and third-party assets are excluded by its `NOTICE`). **No code
was copied.** Patterns only:

| Pattern | Where in ArtCraft | Decision | Here |
|---|---|---|---|
| Page enum with label and breadcrumbs, one routed page at a time | `src/shell.rs` `Page`, `crumbs()` | ADAPT | `state::Workspace`, header breadcrumb |
| Backend work off the UI thread, reported as events drained once per frame | `src/backend/mod.rs` (tokio + `mpsc`), `src/app.rs` | ADAPT, stricter | `loader.rs` (std thread + `mpsc`), plus a ticket on every response, which ArtCraft's events do not carry |
| Theme as named design tokens in one module | `src/theme.rs` | ADAPT | `theme.rs` tokens for light and dark (system-following) |
| egui_kittest tests over the AccessKit tree | `src/overlays/characters.rs` tests | ADOPT (already used here) | `ui_tests.rs` |
| Custom title bar / caption buttons, sidebar chrome | `src/shell.rs` | DEFER | native window decorations kept |
| AI provider clients, auth, Wry sign-in window, media cache | `src/backend/*` | NOT ADOPTED | out of scope |
| Fonts (Inter, Archivo, Geist Mono), brand, icons | `assets/` | NOT ADOPTED | egui default fonts |
| egui/eframe 0.36 + `glow` | `Cargo.toml` | NOT ADOPTED | egui 0.37 + wgpu kept |

craft-libs (`b4afa6d`): its root `Cargo.toml` `[patch.crates-io]` redirects
`egui`, `eframe`, `egui-wgpu`, `epaint` and the other egui crates to a
Storytold egui fork (`6d518fc`), plus `winit` to a Storytold fork. Nothing from
craft-libs is a dependency here. The shell's tab strip is a small internal
widget (`tabs.rs`), so `craft-ui::tabs` could replace it later without touching
pages, if the separate no-patch spike shows it compiles against published egui
0.37.

## Keyboard

| Keys | Action |
|---|---|
| Ctrl+1 … Ctrl+5 | Model, Results, Compare, History, Reports |
| Ctrl+PageDown / Ctrl+PageUp | next / previous workspace (wraps) |
| Left / Right while a tab has focus | activate and focus the neighbouring tab |
| E / I (Model only, no text field focused) | toggle Explorer / Inspector |
| Ctrl+click in the explorer | add or remove one entity in the selection |

## Precautions, not guarantees

- **Accessibility**: the strip is a `TabList` and each workspace a `Tab` with
  `selected` in egui's AccessKit tree, and the headless tests assert exactly
  that. How Windows Narrator or other screen readers announce it is **not
  verified**.
- **WebView2 (APP-01C)**: a WebView2 child window is drawn above egui. The
  design keeps Results selectors and their popups above the chart area and
  expects the WebView to be hidden when its tab is not active or an overlay is
  open. These are design precautions; z-order, focus and popups need physical
  Windows tests at 100/125/150 %.
- No GPU, WebView or Windows test was run for APP-01A (Linux container, no
  adapter).

## Test receipts (agent-reported)

_Filled from the clean-worktree gate; see `evidence/app-01a/`._

## Owner Windows check (public data only)

```powershell
$env:CARGO_TARGET_DIR = ".codex-target"
cargo test --locked --offline -p talos-native
cargo run --release --locked --offline -p talos-native
```

1. The app opens on **Model** with `Public demo project › main › v2` and a
   `Synthetic demo` tag. No warning band. Status line: `Read-only · ETABS not
   connected`.
2. Click each tab, then use Ctrl+1…5 and Ctrl+PageDown/PageUp. Only the active
   tab's page is visible. Compare, History and Reports say why they are
   unavailable and have no controls.
3. Model: press E and I, then switch to Results and back. The panels stay as you
   left them. In the explorer, click slab `S`: the inspector shows `TOP`,
   `FlatAtDeclaredFace` and `0.1524 m (0.5 ft)`.
4. Switch the header version to `v1`. The selection clears. Select `S` again:
   `MIDDLE`, `CentredMiddle`.
5. Results: change Metric, direction and story range, switch tabs and back. The
   query line keeps the choices. The chart area says no analyzed results exist.
6. Focus a tab with Tab, then use Left/Right.
7. Repeat steps 1–3 at 100 %, 125 % and 150 % display scaling and after
   resizing to the minimum window size. Record PASS/FAIL, the Windows build and
   the GPU.
