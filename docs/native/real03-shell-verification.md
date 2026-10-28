# REAL-03 — source-backed shell thickness and insertion (2026-10-28)

**Status: draft, experimental `egui_test` only. HOLD.** All results below are
**agent-reported** from one Linux container (no GPU, no Windows) and await
independent review. EtabExtension #467 remains HOLD. No private model data is
included; a private model was evaluated locally only and only qualitative,
non-derived conclusions are stated.

| Item | Value |
|---|---|
| Base | `main` @ `4f89a4588907e77d7e895aea087be046cd9e8acb` (merged #47) |
| Code head | `16ba799dc0e12a2d786bbb2766a1537bf864c241` (review repair; first review at `7521511`) |
| Backend pin | EtabExtension `9eb1c8792a1d41f3f81c60428afe89f1655782ba` (unchanged) |
| `Cargo.lock` / manifests | unchanged |
| Toolchain / host | Rust 1.99.1; Linux 6.18, 4 vCPU, no GPU; `--locked --offline -j 2`, clean detached worktree |

## Source authority used (not re-parsed)

- `ext-e2k` `ShellPolygon.thickness` / `.insertion_point` / `.opening` and
  `ShellPropDef::effective_thickness()` → `ShellThickness`
  {`Resolved`, `Undeclared`, `Unavailable`, `Ambiguous`}.
- `ext-api` `model_inspect/fidelity.rs::classify_shell` for the canonical shell
  fidelity tier and reasons; `shell_openings.rs` for host attribution.
- Thickness is chosen by the **property definition**, never by Floor/Panel kind
  or polygon orientation.

## Stop condition reached for TOP/BOTTOM

Correct TOP/BOTTOM placement needs to know which side of the connectivity ring
the shell body occupies, that is, the sign of the ETABS shell **local-3** axis
for that object. At the pin:

- `ShellPolygon` exposes no shell local axes and no reversed-local-3 flag;
  `ext-e2k` parses neither from `AREAASSIGN`.
- `ext-e2k`'s own projection documents the wall ring order `[A, B, B, A]` as
  "pending visual validation", so ring winding is not certified to equal ETABS
  joint order for panels.
- No backend module, test or document in the repository establishes the
  TOP/BOTTOM ↔ local-3 face convention for shells.

Therefore only the **certified subset** is implemented: MIDDLE extrusion, which
is symmetric about the ring and needs no local-3 sign. TOP/BOTTOM are drawn as
the flat ring (a true face of the body) and labelled; absent/unknown insertion
is never assumed MIDDLE. **Backend contract decision requested:** expose a
certified shell local-3 direction (or a documented TOP/BOTTOM offset vector) on
the scene contract, validated against ETABS for horizontal, vertical and
inclined shells including any reversed-axis assignment.

## Source-fidelity matrix (native display vs canonical backend)

| Source state | Native display | `ShellPlacement` / `ThicknessSource` | Canonical `ext-api` (mirrored) | Backend viewer draws | Difference |
|---|---|---|---|---|---|
| Resolved t, MIDDLE | **Closed body ±t/2** | `CentredMiddle` / `Resolved` | Exact `[]` | ±t/2 about ring | none |
| Resolved t, TOP or BOTTOM | Flat ring (a true face) | `FlatAtDeclaredFace` / `Resolved` | Approximate `[ShellInsertionPointIgnored]` | ±t/2 about ring (wrong place) | native omits the body rather than misplace it |
| Resolved t, insertion absent or unknown token | Flat ring | `FlatInsertionUnproven` / `Resolved` | Approximate `[ShellInsertionPointIgnored]` | ±t/2 about ring | native omits the body |
| No section / missing definition | Flat ring (body unknown) | `FlatBodyUnresolved` / `NoSection`, `DefinitionMissing` | Exact `[]` | flat | none |
| Zero / negative / absent fields | Flat ring (the whole modeled surface) | `FlatNoDeclaredBody` / `Undeclared` | Exact `[]` | flat | none |
| Conflicting slab vs wall fields | Flat ring (body unresolved) | `FlatBodyUnresolved` / `Ambiguous` | Approximate `[ShellThicknessAmbiguous]` | flat | none |
| Deck / unmodeled family | Flat ring (body unresolved) | `FlatBodyUnresolved` / `FamilyUnmodeled` | Approximate `[ShellThicknessFamilyUnmodeled]` | flat | none |
| Resolved but outside 1e-7..20 m | Flat ring (body not rendered) | `FlatBodyUnresolved` / `OutOfRange` | Exact `[]` (backend tier unchanged) | ±t/2 | native omits the body; inspector says so |
| Declared void (`OPENING "Yes"`) | **Withheld** | `Withheld` | Approximate `[ShellOpeningRenderedAsSolid]` | solid | native never fills a void |
| Backend host of a void | **Withheld** | `Withheld` | Approximate `[ShellOpeningNotSubtracted]` | solid, no hole | native never fills a void |
| Same-story partial overlap | **Withheld** (`OverlapsUnsubtractedOpening`) | `Withheld` | Exact `[]` | solid | intentional, native stricter |
| Void on another story identity | drawn normally | as row 1 | as row 1 | as row 1 | none |
| Invalid ring (self-intersecting, warped, degenerate) | Withheld (`InvalidShellRing`) | `Withheld` | per backend | drawn | native withholds |

The shell `fidelity`/`reasons` fields are a **mirror** of the backend
classifier, never upgraded by native drawing, and the inspector labels them
**backend fidelity**: they describe the backend's own drawing. What the native
viewer actually displays is a separate line, `native display: …`
(`ShellPlacement::native_display`), which never reads as reproduced when a body
was omitted. For example, a MIDDLE shell whose thickness is out of the drawable
range shows `native display: flat ring only: body unresolved or not rendered`
next to `backend fidelity Exact []`. `thickness_source` (what the
definition says) and `thickness_drawn` / `placement` (what is drawn) are
reported separately.

## Before / after (public fixtures)

| Case | Before (`4f89a45`) | After (`7521511`) |
|---|---|---|
| MIDDLE slab or wall with thickness | zero-thickness surface; no shell fidelity fields | closed body at hand-computed extents; Exact `[]` |
| TOP/BOTTOM slab with thickness | zero-thickness surface | flat ring, `FlatAtDeclaredFace`; Approximate `[ShellInsertionPointIgnored]` |
| Base-story column | no `DegenerateExtrusion` reason (parity gap) | `DegenerateExtrusion`, matching the backend |
| Inspector | thickness shown as "(not drawn)" | thickness source, placement, drawn/not drawn, canonical reasons |

## Exact-head test receipts (agent-reported)

**After the review repair**, clean-worktree gate @ `16ba799`
(`evidence/real03-shells/gate-16ba799.txt`): check, fmt, doc tests and WASM
pass; tests **120 passed, 0 failed, 6 ignored** (GPU-only, not run);
whole-workspace strict Clippy exit 101 from the recorded `webview_stage_a`
exception only, every other target clean. The 22 fixtures are now committed
(a narrow `.gitignore` exception; they were silently ignored at `a78a30e`).

The original receipt for the first review follows.

Clean-worktree gate @ `7521511` (`evidence/real03-shells/gate-7521511.txt`):

- check, fmt, doc tests and WASM: **pass**.
- Tests: **117 passed, 0 failed, 6 ignored**. The 6 are GPU-only probes; they
  were **not run** here (no adapter).
- Strict whole-workspace Clippy **returns 101** because of the recorded
  pre-existing `webview_stage_a` Linux-only `eprintln!` exception (G-ENV-1).
  Every other target, linted explicitly, is clean. This is not an
  unconditional Clippy pass.

New `lab-e2k-mesh` tests (`shell_bodies`), all with hand-computed expectations
(ft × 0.3048, tolerance 1e-5 m) and negative assertions:

1. MIDDLE slab and MIDDLE wall: world bounds, closed-body volume
   (divergence theorem), 12 triangles, winding and unit normals, statuses.
2. TOP, BOTTOM, absent and unknown insertion: flat at the unmoved ring, never
   extruded, canonical reason kept.
3. Reversed ring and concave dart: same bounds, positive volume = area × t.
4. Seven thickness states, each distinct and flat, with no invented thickness.
5. Void, host, same-story partial overlap and other-story void.
6. Picking: top face and side face at exact distances, and a foreground column
   occluding the slab side; wall back face.
7. Geometry identity changes with thickness, is stable for the same bytes, and
   frame bodies are byte-identical.
8. Base-story column carries `DegenerateExtrusion`.

Mutation checks: extruding TOP as MIDDLE fails 2 tests; inward side walls fail
the volume test.

**Oracle parity** (`evidence/real03-shells/public-parity.txt`): a scratch
harness running the real pinned `ext-api` scene-packet path (no ETABS) over 22
public fixtures (`evidence/real03-shells/fixtures/`, sha256 listed) gives
**54/54 frames and 48/48 shells identical** in tier and ordered reasons, plus
the one intentional stricter partial-overlap case. A local private-model run
also showed no frame or shell classification difference; results kept local.

## Remaining limitations

- TOP/BOTTOM bodies are not drawn (blocked on shell local-3 authority, above).
- No hole subtraction; declared voids and their hosts or overlaps stay withheld.
- Shell local-axis rotation, offsets and modifiers are not represented.
- Picking is CPU ray casting against submitted triangles; no GPU ID-buffer
  cross-check. Camera motion never rebuilds the mesh (the GPU mesh key is the
  geometry hash), but buffer reuse is only proven by the opt-in GPU tests.
- Circular and steel frame profiles are unchanged (separate follow-ups).

## Owner Windows acceptance (public fixtures only)

Record PR head SHA, Windows build, Rust version, GPU/driver and PASS/FAIL per
step. Publish nothing derived from private models.

```powershell
$env:CARGO_TARGET_DIR = ".codex-target"
pwsh ./check.ps1 -Offline
cargo test --locked --offline --example native_e2k --features real-workbench -- --include-ignored
$fx = "docs/native/evidence/real03-shells/fixtures"
cargo run --release --locked --offline --example native_e2k --features real-workbench -- "$fx/middle.e2k"
cargo run --release --locked --offline --example native_e2k --features real-workbench -- "$fx/top.e2k"
cargo run --release --locked --offline --example native_e2k --features real-workbench -- "$fx/scoped.e2k"
```

1. `middle.e2k`: the slab and the wall show visible thickness. Orbit and pan to
   a grazing angle: there are top, bottom and side faces with no gaps or
   inside-out (dark) faces. Click the top face, then a side face: the inspector
   shows `Resolved, drawn ±t/2 about ring`, `CentredMiddle`, `Exact []`.
2. Orbit so the column is in front of the slab edge; click the column: the
   column is selected, not the slab.
3. `top.e2k`: the slab is a flat surface at the roof level. The inspector shows
   `Resolved, not drawn`, `FlatAtDeclaredFace`,
   `Approximate [ShellInsertionPointIgnored]`. The wall still has thickness.
4. `scoped.e2k`: the void and the host slab are absent. Independent floor `R`
   and wall `W` are visible.
5. Repeat step 1 at display scaling 100 %, 125 % and 150 %, and after resizing
   the window.
6. Optional, local only: open a **copy** of a committed version's E2K and
   confirm that MIDDLE walls show thickness and TOP slabs are flat. Record
   PASS/FAIL only.
