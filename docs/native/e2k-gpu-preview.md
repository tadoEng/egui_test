# Read-only E2K GPU preview — REAL-02 stage A

This optional example consumes `lab-e2k-mesh` from stacked PR #40 and feeds its owned vertex/index arrays into the existing shared `RendererCore`/`ViewportCallback`. Build with `cargo build --locked --offline --features real-geometry --example native_e2k`. Run the produced executable with one explicit `.e2k` path. `--check-only <export.e2k>` performs only bounded read/conversion and never opens a window. There is no default/persisted model path, recursive discovery, ETABS/sidecar launch, result reader or project write.

The preview opens the canonical selected file read-only, holds a cooperative Windows FILE_SHARE_READ-only handle during preparation, limits it to64MiB, then compares length/mtime/path resolution and reread bytes. This detects ordinary load-time changes; it is not a hostile reparse or continuous-freshness guarantee. Source path/details and normal private runs are not exported. Loading/conversion currently happen synchronously before window creation; cancellation, async switching and last-good recovery are separate work.

One owned vertex array and one owned u32 index array are moved into the renderer without copying their storage. Two resident geometry buffers serve one indexed draw record per callback; the camera has a separate64-byte uniform buffer. The complete32-byte geometry digest keys upload reuse. Camera and comment-only provenance changes retain geometry buffers. Geometry identity is not project/version/selection authority. This first preview drops entity/source metadata after the summary and ownership transfer; no picking or story filtering is implemented.

Frame bodies are0.1m rectangular display proxies; supported planar convex shells are zero-thickness surfaces. Source sections, local-axis roll, offsets/insertion, opening subtraction, group and Wall/Slab semantics are unverified. Withheld/unknown shell surfaces are not silently filled. Persistent native labels disclose these limits and unverified source/analysis freshness. Cached results are not attached in this slice. Public conversion provenance remains documented in the crate README.

## Evidence modes

`native_e2k --public-smoke` accepts no user path and loads only the independently authored `lab_e2k_mesh::public_fixture::E2K`. It opens an owned hidden native host, waits for GPU callback draw records, emits one JSON stdout receipt and closes. A caller must require readable JSON, assessment PASS and exit0; exit alone is insufficient. This proves callback integration only, not presented frames, physical interaction or clean-machine packaging.

The explicit ignored `public_e2k_upload_readback_and_geometry_cache` test renders that same public fixture offscreen, checks nonclear GPU readback and resident buffer reuse/replacement, and optionally saves a public image to `E2K_PUBLIC_GPU_CAPTURE`. It never accepts private input. `offscreen_gpu_depth_keeps_the_nearer_face` retains the prior independent depth oracle. These are GPU tests, not engineering-source parity or physical Windows UX evidence.

### Smoke exit contract — review follow-up

The callback now sends its completed verdict through an owned bounded channel, and `run()` checks it after native-window teardown. Only a passing GPU counter verdict **and** successfully serialized/flushed stdout receipt can return success. FAIL, timeout, early close/missing verdict and receipt-write failure return an error from `main`, producing a nonzero process exit. Timeout cannot pass merely because draw records arrived after the deadline. This fixes the earlier behavior in which an external caller had to catch a FAIL receipt despite exit0; prior published PASS receipts already required both JSON PASS and exit0.

Two explicit negative controls accept **no input path** and use only the same authored fixture:

```powershell
native_e2k.exe --public-smoke                    # require JSON PASS + exit0
native_e2k.exe --public-smoke-force-fail         # require JSON FAIL + nonzero exit
native_e2k.exe --public-smoke-missing-receipt    # require no receipt + nonzero exit
```

The latter two exercise actual post-window failure propagation; they do not alter GPU counters, source geometry or production inputs. Extra arguments/private source paths are refused for every public mode. Unit controls cover false/missing/true verdicts and a broken stdout flush. **These new controls are implemented but not compiled/executed yet:** disk headroom remains below the unchanged6GiB guard. Formatting, locked offline metadata and whitespace checks only are verified for this follow-up. Historical GPU/native receipts below remain tied to their original source and do not validate the new integrated head. After headroom recovers, repeat the exact-head canonical native/WASM/Clippy/plain-Cargo checks and public GPU/CLI/positive/negative smoke controls before requesting acceptance. No merge or native-migration readiness is implied.

Counters distinguish buffer-creation host CPU time from GPU completion. `draw_indexed_records` counts encoded draw calls, not presented frames/FPS; vertex/index byte counters are logical resident buffer sizes, not total VRAM or process RSS. No Debug timing is a throughput benchmark.100k loading, selection, async lifetime/cancel, runtime network capture and manual DPI/focus remain pending. All six migration gates stay open, #18 CRT/package HOLD remains separate, and Tauri remains production.

## Owner manual check

Launch with an explicit E2K path; drag left/right and up/down to orbit, scroll to zoom, and use Fit. Confirm visible approximation and withheld-count labels, resizing and fit at narrow/wide aspect. Do not publish private screenshots/coordinates/counts, timings, names or hashes. The agent does not automate these physical interaction checks.

## Exact-source Windows verification

At code `d5f7b8e358da6c40f74d43a16a067229a5b761d1`, full offline verification passes98 CPU test executions (19 shared repetitions),6 opt-in GPU instances ignored, fmt/strict Clippy/native all-target/all-feature/docs0/WASM. Separate legacy `native-renderer` feature-only strict Clippy passes. The two explicitly executed GPU tests pass on the RTX3050Ti (E2K source readback/cache on Vulkan driver591.74; independent depth oracle on Dx12, whose driver text was unavailable). Both public images were inspected. The source-geometry test checks actual mapped pixels and same-buffer reuse for comment-only provenance changes; changing the last digest byte replaces both geometry buffers.

The standalone Debug executable is22,014,464bytes, SHA256 `6df00df05853125bbcd02d60446bdd4d411b21a59bb48d36fed39b45c3f842c3`. Its owned hidden public smoke passes with52vertices/26triangles,1upload,2resident geometry buffers,1,872vertex bytes/312index bytes and3draw/callback records. CPU buffer creation14,800ns is a single Debug observation, not a GPU upload benchmark or performance result. CLI public check-only exits0; no-args/EDB/public-smoke-plus-path each exit1. Owned fixture bytes/mtime remain unchanged, the temporary fixture directory is removed and each owned root process exits. No WebView is created by this standalone example; this does not close browser or worker lifecycle gates.

[Source, log hashes and scope](evidence/real02-gpu/windows-validation.json), [native/CLI receipt](evidence/real02-gpu/public-native-validation.json), [hidden host report](evidence/real02-gpu/public-host-smoke.json), [public source readback](evidence/real02-gpu/public-e2k.png), [depth oracle](evidence/real02-gpu/depth-probe.png). Final evidence changes preserve compiled inputs. Overall REAL-02 remains PARTIAL: source geometry preview only, not the one-window same-version results/selection workbench. No private evidence is included.
