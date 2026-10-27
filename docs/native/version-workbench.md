# REAL-02: one checked version workbench

**PARTIAL / build validation pending.** This source slice is stacked on #41 at `78cf0164fa52bb53a680df498a77ff8eec87677d`. Its parent incorporates owner-merged #35, #39 and #40 from main `1b9aab7bfc4c03b3052249d2ca97abbc5f2ee976`. No PR was merged by this implementation. Formatting, locked offline metadata and whitespace checks pass; the new Rust code, GPU overlay and native smoke have not been compiled/executed yet because D: is below the existing 6 GiB free-disk guard. Prior #41 GPU evidence belongs to its earlier exact source, not this code.

## Data path and authority

The optional `real-workbench` feature selects the existing `real-geometry`, `snapshot-preflight` and `webview` features. The Windows `native_workbench` example requires an explicit version directory; it has no default project, persisted input, discovery scan, live ETABS, calculation or version-write path. Non-Windows/WASM entry points are stubs. No dependency/pin/lockfile upgrade or ext-api/V8 path is added. The authoritative backend remains `9eb1c8792a1d41f3f81c60428afe89f1655782ba`.

| Component | Source reused | Combined-host responsibility |
| --- | --- | --- |
| DTOs, canonical unit/provenance checks and signed/gapped plot values | #35 `examples/native_results/data.rs`, pinned ext-calc/ext-core | Build the same `Snapshot` from the exact bytes held by preflight; preserve stored order/values |
| Snapshot/E2K/metadata checks | #39 `examples/native_results/preflight.rs`, pinned ext-e2k/ext-sidecar DTO | Retain all read handles through mesh/packet preparation, then repeat path/time/byte/partial and relevant-entry checks |
| Geometry conversion and source attribution | #40 `crates/lab-e2k-mesh` | Consume its validated mesh without recreating parser or engineering domain logic |
| GPU upload/cache/depth/orbit | #41 `examples/support/native_viewport.rs` | Move arrays plus EntityKey/story/index ranges; add one bounded selected-story index overlay |
| Offline chart style, packet and WebView transport | #35 chart template; existing bounded `ChildWebView` | One child with two ECharts, native story/table selection, same-scope typed IPC |

The existing standalone reader/preflight commands keep their behavior. A byte-based DTO constructor is shared rather than deserializing domain types again. This also closes a combined-load ambiguity: preflight no longer reopens joined paths through `Snapshot::load` to construct DTOs that could differ from its held inputs.

This is load-time cooperative integrity, not hostile filesystem/reparse security certification. Each artifact is bounded to 64 MiB; immediate version enumeration remains bounded to 512 entries. The combined host requires one unambiguous in-directory E2K and confirmed compatible units. It does not attach an external export by assumption. Source-current certification and analysis freshness remain false/unverified even when all checks pass. Manifest analysis flag, optional metadata observation/coherence and stored engineering QA are displayed separately; cached QA is not execution proof.

## Version scope and selection

The private in-memory scope hashes the canonical selected-directory namespace, exact held result/manifest/unit/optional metadata bytes and E2K bytes. Equal `main/v1` labels and equal files in two projects therefore cannot cross-select. This token is identity for this loaded context, not an engineering provenance certificate. No user-derived scope/hash is exported in a public artifact.

Chart packets carry that scope and a monotonic generation. Direction/case/group changes clear selection and increment generation; A→B→A cannot revive old IPC. Native story/table and chart selections require exact membership in the source story list and **both** stored plot orders. Unknown/case-folded names, stale scopes/generations, oversized messages and malformed schemas are refused. Geometry-only or one-plot-only stories remain visibly disabled for shared selection. There is no group inference, member-force join, picking or contour calculation.

Gold geometry uses only the converter's source-assigned story and entity index ranges. A column attributed to its upper source story can extend below that story. Highlighting is not a spatial storey cut. The original geometry's vertex/index buffers remain resident; a selection change builds at most one additional 64 MiB-bounded index buffer and one overlay draw, not one buffer/draw per member. Unchanged selection/camera callbacks reuse it. Clear selection releases that overlay. Geometry cache identity is separate from project/version/IPC identity.

**Recheck selected version** checks the same explicit directory again. A changed/unreadable/partial version retains the last valid geometry and stored values, clears selection, expires its generation and displays UNLINKED in native/chart regions. Linked selection remains disabled even if old bytes return. Reopen explicitly to adopt a snapshot. The host does not silently replace immutable versions. Checking happens at load and on this explicit action; continuous filesystem monitoring is not claimed.

## Build and owner launch

Do not bypass resource guards. Require at least 6 GiB available RAM and 6 GiB free disk at build launch, retain the 2 GiB available-RAM floor during owned builds, jobs1 and Rust stack268435456. Use installed sccache/nextest per #43, process-local owned D: compiler-cache/temp/server settings and **this physical worktree's own** `.codex-target`. No downloads or borrowed target folders. sccache previously fell back to compilation in #35; no acceleration claim follows from enabling it.

After the guarded build and checks pass:

```powershell
# Run from the feature worktree; replace the placeholder explicitly.
cargo run --locked --offline --release --target-dir .codex-target --features real-workbench --example native_workbench -- '<version-directory>'

# No window or public output; success means only the supported load-time checks.
cargo run --locked --offline --release --target-dir .codex-target --features real-workbench --example native_workbench -- --check-only '<version-directory>'
```

Use native source stories or stored rows to update both chart markers and attributed geometry. A chart point should update the same native selection. Exercise X/Y and case/group contexts, clear, fit/orbit, resizing and recheck. Report owner observations as owner observations. Do not upload private screenshots, values, names, paths, counts, hashes, timings or warnings. Frame proxies, zero-thickness supported shells, withheld unknown/void surfaces and unverified axes/sections/offsets/insertion/opening subtraction remain explicit limitations.

## Pending exact-source checks

1. Canonical `check.ps1 -Offline`: fmt, native all targets/features, strict Clippy, plain Cargo tests, doc tests, WASM. Preserve original warnings.
2. Additional nextest discovery/count parity and single-thread pilot; do not replace plain Cargo/doc/GPU checks.
3. Explicit public GPU depth/cache and story-overlay readback. Inspect only public fixture captures; count uploads/encoded callbacks separately from presented frames.
4. Guarded exact-source `native_workbench` build and owned hidden `--public-workbench-smoke`; require readable JSON assessment PASS **and** exit0. The mode accepts no path. It creates only an authored Roof/L1 DTO + E2K fixture, checks native→both markers and typed IPC→native, stale-generation refusal, resident mesh/overlay counters and input removal. Programmatic JS messages are not physical chart input.
5. Owned public `--check-only` success and noargs/bad-directory/public-mode-plus-path refusals, unchanged fixture bytes/mtime; legacy renderer/dashboard/results feature regressions.

New authored tests cover exact-byte DTO parity, cross-project scope, changed E2K identity with identical geometry, A→B→A, common-story membership, last-good/unlink, cooperative read guards, optional metadata/export-set changes, version mismatch and exact bounded highlight ranges. These are **written, not yet executed**. No test count/PASS is claimed before the actual logs exist.

Normal input loads and rechecks are synchronous; async cancellation/stale completion and worker join remain open. WebView incognito/parent exit does not establish isolated profiles, browser-process cleanup or runtime RSS. Manual Windows chart input/focus/DPI/z-order, network capture, 20× lifecycle, meaningful project readiness, packaging and engineering parity remain open. No presented FPS/latency/VRAM claim is made. #18 CRT/package HOLD remains separate; all six migration gates stay open and Tauri remains production. Stress conversion #42 stays deferred.
