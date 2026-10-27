# Read-only E2K GPU preview — REAL-02 stage A

This optional example consumes `lab-e2k-mesh` from stacked PR #40 and feeds its owned vertex/index arrays into the existing shared `RendererCore`/`ViewportCallback`. Build with `cargo build --locked --offline --features real-geometry --example native_e2k`. Run the produced executable with one explicit `.e2k` path. `--check-only <export.e2k>` performs only bounded read/conversion and never opens a window. There is no default/persisted model path, recursive discovery, ETABS/sidecar launch, result reader or project write.

The preview opens the canonical selected file read-only, holds a cooperative Windows FILE_SHARE_READ-only handle during preparation, limits it to64MiB, then compares length/mtime/path resolution and reread bytes. This detects ordinary load-time changes; it is not a hostile reparse or continuous-freshness guarantee. Source path/details and normal private runs are not exported. Loading/conversion currently happen synchronously before window creation; cancellation, async switching and last-good recovery are separate work.

One owned vertex array and one owned u32 index array are moved into the renderer without copying their storage. Two resident geometry buffers serve one indexed draw record per callback; the camera has a separate64-byte uniform buffer. The complete32-byte geometry digest keys upload reuse. Camera and comment-only provenance changes retain geometry buffers. Geometry identity is not project/version/selection authority. This first preview drops entity/source metadata after the summary and ownership transfer; no picking or story filtering is implemented.

Frame bodies are0.1m rectangular display proxies; supported planar convex shells are zero-thickness surfaces. Source sections, local-axis roll, offsets/insertion, opening subtraction, group and Wall/Slab semantics are unverified. Withheld/unknown shell surfaces are not silently filled. Persistent native labels disclose these limits and unverified source/analysis freshness. Cached results are not attached in this slice. Public conversion provenance remains documented in the crate README.

## Evidence modes

`native_e2k --public-smoke` accepts no user path and loads only the independently authored `lab_e2k_mesh::public_fixture::E2K`. It opens an owned hidden native host, waits for GPU callback draw records, emits one JSON stdout receipt and closes. A caller must require readable JSON, assessment PASS and exit0; exit alone is insufficient. This proves callback integration only, not presented frames, physical interaction or clean-machine packaging.

The explicit ignored `public_e2k_upload_readback_and_geometry_cache` test renders that same public fixture offscreen, checks nonclear GPU readback and resident buffer reuse/replacement, and optionally saves a public image to `E2K_PUBLIC_GPU_CAPTURE`. It never accepts private input. `offscreen_gpu_depth_keeps_the_nearer_face` retains the prior independent depth oracle. These are GPU tests, not engineering-source parity or physical Windows UX evidence.

Counters distinguish buffer-creation host CPU time from GPU completion. `draw_indexed_records` counts encoded draw calls, not presented frames/FPS; vertex/index byte counters are logical resident buffer sizes, not total VRAM or process RSS. No Debug timing is a throughput benchmark.100k loading, selection, async lifetime/cancel, runtime network capture and manual DPI/focus remain pending. All six migration gates stay open, #18 CRT/package HOLD remains separate, and Tauri remains production.

## Owner manual check

Launch with an explicit E2K path; drag left/right and up/down to orbit, scroll to zoom, and use Fit. Confirm visible approximation and withheld-count labels, resizing and fit at narrow/wide aspect. Do not publish private screenshots/coordinates/counts, timings, names or hashes. The agent does not automate these physical interaction checks. Public runtime/source/artifact evidence will be recorded separately after execution; this document does not itself assert a completed GPU run.
