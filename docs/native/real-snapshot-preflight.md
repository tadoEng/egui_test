# REAL-01 read-only version preflight

This optional Windows CLI mode checks an explicitly selected snapshot without starting a window, ETABS, a sidecar client, a calculation or a version operation. It does not modify any selected artifact, manifest, sentinel, database or Git state.

```powershell
cargo run --locked --offline --features webview,snapshot-preflight --example native_results -- --preflight '<version-directory>'
```

Output contains **categorical status codes only**: geometry readiness, authoritative result DTO validity, supported plots, observed/manifest analysis flags, metadata chronology, engineering summary status, declared units, story-key/order coherence, load-time integrity and fidelity limitations. It never emits user paths, filenames, IDs, story/group names, values, counts, warnings, hashes or timings. Keep even categorical private run output local; public evidence uses the independently authored fixture only.

An exit0 permits only a clearly labelled **uncertified preview** of consistent stored inputs. `same_snapshot_linkage` remains unverified and `source_current_certified` is always false. Shared folder placement, matching units/story labels, sizes and load-time bytes do not bind cached results to a source-model analysis. Explicit mismatches refuse pairing; result DTO readiness remains separately reported. A cached engineering FAIL is not analysis execution failure. Earlier observational metadata or a false analyzed flag does not invalidate otherwise valid cached values or cause a write-back.

## Source authority and dependency scope

One immutable EtabExtension pin: `9eb1c8792a1d41f3f81c60428afe89f1655782ba`.

- [CalcOutput / CalcMeta](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-calc/src/output.rs), [SourceUnitMetadata](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-calc/src/source_units.rs) and [VersionManifest](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-core/src/version/manifest.rs) reuse the existing reader validation.
- [ModelMetadata / StoryInfo](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-sidecar/src/commands.rs) are deserialized directly. The optional ext-sidecar dependency is **DTO reuse only**. No SidecarClient construction, transport/process method, COM or operation API is called; this is not INT-05 lifecycle acceptance.
- [ext-e2k parsing/source maps](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-e2k/src/lib.rs), [unit parsers](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-e2k/src/units.rs), and [project_scene](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-e2k/src/scene.rs) supply model/connectivity authority. A rejecting UNITS-header guard is needed because the pinned parser silently retains defaults for unknown tokens. It uses the authoritative unit parsers and does not parse engineering geometry itself.
- Chrono directly uses the already locked0.4.41 for timestamp comparison. The lock adds one same-pin ext-sidecar record; resolver changes among existing versioned dependency edges are checked by the combined native/WASM gate. No ext-api/V8/Deno/Typst closure or production change.

## Explicit file scope and refusal

Only immediate entries in the user-selected directory are considered (bounded512 entries), without recursive project discovery or ambient directory scanning. Zero E2K files yields `absent_edb_is_not_e2k`; multiple files require explicit selection. EDB existence is reported but its binary contents are never opened/parsed. A nested export must be selected explicitly.

```powershell
# Select one export inside the version:
& '<native_results.exe>' --preflight '<version-directory>' --e2k 'nested/model.e2k'
# An external export additionally requires explicit opt-in and expected SHA256:
& '<native_results.exe>' --preflight '<version-directory>' --e2k '<external.e2k>' --allow-external-e2k --e2k-sha256 '<expected-sha256>'
```

The hash is checked locally and is never emitted. An external export remains `external_unverified` and cannot automatically link to the results. No EDB export is attempted when E2K is absent.

Artifact reads are read-only, bounded64MiB each, open the canonical verified target, retain FILE_SHARE_READ-only Windows handles, and recheck path resolution, metadata and bytes before success. Escapes outside the selected root, partial versions, incompatible identity/units, nonfinite projected coordinates and unknown/duplicate/wrongly ordered result stories are refused. Source order may be an explicitly validated subset of the E2K order. This is cooperative load-time protection, **not a hardened hostile directory/reparse TOCTOU boundary** or continuous post-load freshness guarantee.

Metadata stays observational: collected time and analyzed flag are distinct from manifest state, CalcOutput generation time and engineering QA. Geometry uses the pinned projection's declared approximations/skips; section placement, local axes, offsets, openings, groups and per-member results are not certified by this preflight. No member quantities or new engineering envelopes are inferred.

## Public validation

```powershell
& '<native_results.exe>' --public-preflight-probe
```

This separate mode accepts no source/report path. It creates only the owned LAB-REAL01-01-r1 fixture, checks it through the same preflight, removes the TempDir and emits its public categorical receipt. Normal preflight never creates a fixture or writes selected files.

Eighteen public unit/negative cases cover the valid signed cache, EDB-only/no E2K, ambiguity, missing/partial/mixed versions, unknown/conflicting units, malformed/unresolved geometry, metadata-before-result/analysis/check discrepancies, missing/gapped/unknown/duplicate/wrong-order stories, external hash/opt-in refusal, stale-size mismatch, cooperative write exclusion, changed-source detection, bounds and categorical output privacy. Full exact-head offline gate and executable evidence must be recorded before claiming those broader checks pass. Windows physical input, GPU scene rendering, analysis-current and migration gates remain open. #38 owns the separately reviewed geometry bridge.
