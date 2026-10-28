# REAL-02: one checked version workbench

**PASS for the authored public composite checks; programme PARTIAL pending independent re-review.** This PR is now based on actual main `aaa05d8dad5229d3aef0a4221b6edfd468fad42a`, after the owner merged #41 and #45. The final tested code is `b9fc0f36ea193bfc9dedebd878e7ccf6fda478ad`. The lockfile is byte-identical to main and the authoritative backend pin stays `9eb1c8792a1d41f3f81c60428afe89f1655782ba`. No GitHub PR was merged by this implementation. Strict-check fixes extract renderer pipeline creation and simplify two host expressions without suppressing warnings.

The main-based native/Clippy/plain-Cargo/doc gate passes 189 CPU test executions (including shared repetitions; 12 opt-in GPU instances ignored). The mandatory WASM command passes separately with the process-local compiler wrapper unset because sccache previously demonstrated Windows spawn error206 on `web-sys`. Native commands use the installed sccache; warnings remain visible. Additional nextest passes 189 tests with 12 skipped; 201 listed and matching test-name/ignored-name multisets. This split gate is not an uninterrupted `check.ps1 -Offline` exit0 or a cache/performance claim.

Three explicit authored GPU probes pass: selected source-story gold pixels and clearing, unchanged resident-geometry cache, and independent depth. All three public captures were inspected. The actual Debug/debug0 workbench is 26,541,568 bytes, SHA256 `fa98fbb173afd2089205c9f008e17838527c3222c757248cbc2b9e61adfa6edb`.

The owned hidden native workbench returns JSON PASS and self-exits0 after checking native→both chart markers, typed chart→native, a refused stale selection, one geometry upload, source-story highlight draw/clear and owned-input removal. Twelve actual CLI cases pass, including supported load, missing/ambiguous E2K, partial version, mismatched version, malformed cached results and argument refusals; authored file bytes/mtime remain unchanged during every read-only invocation. Model tests also cover A→B→A, different-project equal labels, common-story membership, changed-source last-good retention and persistent UNLINKED state. The r2 public native smoke now verifies persistent unlink in the running WebView too: it creates a `.partial` marker exclusively inside its generated public `TempDir`, checks failed recheck, removes that marker and confirms restored inputs still require reopen. It reads back the UNLINKED notice, disabled chart pointer events, cleared markers and unchanged signed values, then verifies Rust refuses a direct current-generation selection while unlinked. Original manifest/unit/result/E2K bytes and mtimes remain unchanged; no explicit user-version path can construct this writable smoke control. Two extra CPU controls check safe marker handling, including never clobbering an existing marker. Physical UI confirmation remains open.

[Exact-source command/log/resource receipts](evidence/real02-workbench/unlink/windows-validation.json), [native and read-only CLI controls](evidence/real02-workbench/unlink/native-controls.json), [native host receipt](evidence/real02-workbench/unlink/public-host.json), [nextest name parity](evidence/real02-workbench/unlink/nextest-parity.json). These contain only independently authored public fixture facts. Final documentation/evidence changes preserve the tested compiled inputs. The earlier7873/r1 receipts remain historical in the parent evidence folder; they are not reused for this changed smoke. Keep draft and request independent exact-head re-review; no merge or engineering provenance/migration certification is implied.

An additional parameterized-wrapper replay stopped at the unchanged launch-headroom guard after three successful cases. It is not accepted as a second full suite; the primary12-case run above completed. Its attempted wrapper remains local and is not published as a verified reusable harness.
## Data path and authority

The optional `real-workbench` feature selects the existing `real-geometry`, `snapshot-preflight` and `webview` features. The Windows `native_workbench` example requires an explicit version directory; it has no default project, persisted input, discovery scan, live ETABS, calculation or user-version write path. Non-Windows/WASM entry points are stubs. No dependency/pin/lockfile upgrade or ext-api/V8 path is added. The authoritative backend remains `9eb1c8792a1d41f3f81c60428afe89f1655782ba`.

| Component | Source reused | Combined-host responsibility |
| --- | --- | --- |
| DTOs, canonical unit/provenance checks and signed/gapped plot values | #35 `crates/lab-results/src/data.rs` (moved from `examples/native_results/data.rs`), pinned ext-calc/ext-core | Build the same `Snapshot` from the exact bytes held by preflight; preserve stored order/values |
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

## Executed exact-source checks

1. Canonical `check.ps1 -Offline -SkipWeb` PASS plus the identical mandatory WASM command through direct rustc PASS. Preserve original warnings and the recorded wrapper exception.
2. Additional nextest discovery/count parity and single-thread pilot; do not replace plain Cargo/doc/GPU checks.
3. Explicit public GPU depth/cache and story-overlay readback. Inspect only public fixture captures; count uploads/encoded callbacks separately from presented frames.
4. Guarded exact-source `native_workbench` build and owned hidden `--public-workbench-smoke`; require readable JSON assessment PASS **and** exit0. The mode accepts no path. It creates only an authored Roof/L1 DTO + E2K fixture, checks native→both markers and typed IPC→native, stale-generation refusal, resident mesh/overlay counters and input removal. Programmatic JS messages are not physical chart input.
5. Owned public `--check-only` success and noargs/bad-directory/public-mode-plus-path refusals, unchanged fixture bytes/mtime; legacy renderer/dashboard/results feature regressions.

New authored tests cover exact-byte DTO parity, cross-project scope, changed E2K identity with identical geometry, A→B→A, common-story membership, last-good/unlink, cooperative read guards, optional metadata/export-set changes, version mismatch and exact bounded highlight ranges. These tests are included in the actual 189-execution plain-Cargo gate and the additional nextest run; repeated shared example tests are not distinct domain proofs.

Normal input loads and rechecks are synchronous; async cancellation/stale completion and worker join remain open. WebView incognito/parent exit does not establish isolated profiles, browser-process cleanup or runtime RSS. Manual Windows chart input/focus/DPI/z-order, network capture, 20× lifecycle, meaningful project readiness, packaging and engineering parity remain open. No presented FPS/latency/VRAM claim is made. #18 CRT/package HOLD remains separate; all six migration gates stay open and Tauri remains production. Stress conversion #42 stays deferred.
