# CPU stress observations — 2026-10-25

Windows, Rust 1.99.1, egui/eframe 0.37.0, Cargo release profile (`opt-level = 2`),
1200 × 760 logical points. The headless `stress` example drives orbit input on
the actual app, discards three warm-up frames, and samples 30 frames per scene.
No desktop input, native window, or GPU renderer is involved.

| Scene | Entities | Submitted segments | Build/import ms | UI median ms | UI p95 ms | Tessellation median ms | Tessellation p95 ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| Synthetic demo | 48 | 48 | 0.040 | 0.112 | 0.144 | 0.072 | 0.117 |
| Synthetic grid | 50,000 | 50,000 | 8.812 | 2.407 | 3.175 | 11.581 | 14.536 |
| Synthetic grid | 100,000 | 100,000 | 19.721 | 4.408 | 5.200 | 22.422 | 24.408 |

The parser dependency is pinned to EtabExtension revision
`9eb1c8792a1d41f3f81c60428afe89f1655782ba`. Private input files, paths, geometry,
and model-derived benchmark measurements are excluded from this repository.

UI time measures CPU app/UI construction. Tessellation time includes cloning
the submitted shapes and CPU tessellation; triangle counts in the CSV include
the UI chrome. The 100,000-member scene produced about one million triangles.
These are observations from one local run, with other system activity
uncontrolled. They are not presentation FPS, GPU timings, memory usage, a
hardware capacity ceiling, or an equivalence check against the ETABS renderer.

Reproduce synthetic measurements:

```powershell
cargo run --locked --offline --release --features e2k --example stress
```

Reproduce with a local export (substitute your own path):

```powershell
cargo run --locked --offline --release --features e2k --example stress -- --e2k 'C:\local\model.e2k'
```
