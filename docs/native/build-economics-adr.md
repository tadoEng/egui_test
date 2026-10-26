# PERF-02: measured iteration boundary and CRT investigation

Status: **PROPOSED / PARTIAL**. References: [#19](https://github.com/tadoEng/egui_test/issues/19),
[#18](https://github.com/tadoEng/egui_test/pull/18), [#16](https://github.com/tadoEng/egui_test/pull/16),
[#14](https://github.com/tadoEng/egui_test/issues/14). No production source change or shipping
second process is authorized by this ADR. No merge/migration approval follows from measurements.

## Frozen comparison

- Baseline: main `aa28552a23da0e9983ed768ba3e1ccaee7937baa`, egui plus merged offline WebView.
- Backend integration: `cf4fbb218da1e62cf767a17a35e49f9c4f48e50e`, same main plus optional read-only
  adapter. Runtime implementation remains `6336023`; backend API pin is
  `9eb1c8792a1d41f3f81c60428afe89f1655782ba` throughout.
- Windows 11 build 26300, Ryzen 5 6600H (6 cores/12 threads), 16,312,721,408 bytes RAM,
  Rust 1.99.1. One Cargo job; dev/test debug info disabled; dependency dev opt-level 2;
  Release opt-level 2, default thin/fat LTO disabled. No global CRT flags or warning suppression.

The earlier 1762 s check and 3820 s test-codegen observations are not matched cold builds.
Cold below means a fresh **target** directory with a populated shared registry/source cache;
it does not mean an empty OS file cache, network download or fresh machine. Report exact command,
SHA, features, cache class and disk. Do not compare `check` directly with Release code generation.

## Authoritative dependency roots

Actual `cargo tree --locked --offline --features native_backend,webview
--target x86_64-pc-windows-msvc -i <crate> -e features` outputs:

| Heavy component | Enabled route | Evidence |
|---|---|---|
| Typst 0.15.1 | ext-api → ext-report → typst default | [tree](evidence/perf19/typst-roots.txt) |
| Polars 0.53 | ext-api / ext-render / ext-report → ext-calc, dtype-full/parquet/lazy | [tree](evidence/perf19/polars-roots.txt) |
| V8 137.3.0 | ext-api → ext-render default/ssr → Charming 0.6 ssr → deno_core 0.351 → V8 | [tree](evidence/perf19/v8-roots.txt) |
| Deno | Charming's ssr activates deno_core and serde_v8 | [tree](evidence/perf19/deno_core-roots.txt) |

[Pinned ext-api manifest](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/Cargo.toml)
has unconditional calc/render/report/automation dependencies. Disabling its default features
cannot trim those dependencies. Lockfile package count is not compiled-crate count.

## Combined WebView constraint

The integrated lockfile has 1056 cross-platform records, versus the pre-WebView backend's 967.
Wry 0.55.1's GTK/glib closure selects proc-macro-crate 2.0.2, which pins
`toml_datetime = 0.6.3`. Retaining backend `toml` 0.8.23 requires datetime 0.6.11 and fails the
resolver. The combined lock selects `toml` 0.8.2; it is also consumed by Typst on Windows.
This is a real integration difference, even though GTK itself is not compiled on Windows.
Native/WASM success does not certify Linux/macOS. No merged Wry dependency definition is removed.

## CRT root cause and safe disposition

MSVC 14.44.35207 `dumpbin /directives` on the actual cached x64 archives reports:

| Archive | Observed directive |
|---|---|
| `rusty_v8.lib` | `/DEFAULTLIB:libcmt.lib` |
| SQLite, zstd, lz4, ring, stacker native archives | `/DEFAULTLIB:MSVCRT` |

V8 archive SHA-256: `b36ae296e7bbd1069ad74eee533031a3955c2d9ce1556477fe83fa6357e5d8e8`.
The V8 build output identifies upstream asset
`rusty_v8_release_x86_64-pc-windows-msvc.lib.gz` from release `v137.3.0`.
The published v8 137.3.0 build script also requests `msvcprt` in the active configuration.
Its manifest exposes `use_custom_libcxx`, not a user-selectable `crt-static` feature, despite
the script's conditional branch checking that feature environment variable.

This provides a concrete input chain for observed LNK4098. It is not evidence that all archived
objects are loaded or that a particular allocator path is safe. Microsoft documents the warning
as incompatible runtime-library mixing and recommends consistent runtime compilation:
[LNK4098](https://learn.microsoft.com/en-us/cpp/error-messages/tool-errors/linker-tools-warning-lnk4098?view=msvc-170).

**Disposition:** retain the warning; HOLD packaged/native-backend delivery acceptance. A separate
review must select and verify a compatible V8 native artifact or audited source build with a
consistent CRT policy, then test relevant native allocation/error/lifecycle paths and Release
packaging. Do not set global CRT flags, invent a crate feature, use `/NODEFAULTLIB` as a green-log
workaround, or change EtabExtension production. API fixture success is narrower than that policy.

## Reversible options

| Option | Reuse / proposal | Economics and responsibilities | Recommendation |
|---|---|---|---|
| A. Existing optional in-process ext-api | Direct reuse, already implemented | Heavy cold build when native_backend is enabled; authoritative DTOs/errors/guards retained. UI-only Cargo invocation can leave it disabled. Whole-package feature-enabled relinking still needs measurement. | First development choice: retain the feature fence, measure targeted UI commands and serialized heavyweight CI. |
| B. Lightweight project-domain API facade | Proposed optional feature/interface split | Could remove calc/report/SSR closure for read-only builds, but requires upstream manifest/export changes and exact contract/feature-matrix tests. No measured savings yet. | Separate owner-approved EtabExtension proposal/PR only; do not implement here. |
| C. Thin generated contract + separately compiled read-only backend probe | Proposed interface/process split | Keeps heavy code in one authority process but adds IPC/startup/packaging, error chains, version negotiation, one owner and shutdown. Source SceneLoadGuard stays authoritative; no copied UI version/provenance rules. | No shipping process or hand-written replacement DTO/domain layer. Consider an isolated prototype only after A measurements and contract review justify it. |

All options preserve immutable committed versions, source-fingerprint checks, operation gates,
sidecar validation/refusal and explicit ETABS consent. Separating a read-only backend process
does not authorize an ETABS sidecar or external process spawned from repaint. Same-source API
fixture parity is not independent CLI/Tauri/engineering parity.

## Measurement protocol

`scripts/measure-cargo.ps1` launches only its own Cargo process with argument-list escaping,
collects `--timings`, elapsed time, target size/free disk and approximate one-second process-tree
working/private memory. It stops only its own build tree if target-drive headroom falls below
the threshold. It never deletes caches or follows target reparse points when counting bytes.
Raw local reports/logs require sanitization before publication. Fast no-op runs can exit before
memory sampling; unavailable memory is null rather than a fabricated zero.

Use disposable benchmark checkouts for UI/backend edit cases; never edit a reviewed PR to create
a benchmark. Run one heavyweight compilation at a time. Use fresh uniquely named target folders
on the same drive/settings for cold comparison; do not use `cargo clean` on shared targets.
At least two no-op/edit repetitions are feasible; cold repetition depends on disk/time. Record
single runs honestly. Startup must name its readiness event; process launch or cargo completion
alone is not an interactive-host readiness measurement.

## Evidence ledger

- Dependency roots and CRT directive chain: **PASS for inspected source/artifacts**.
- Integrated first `check --all-targets --all-features --timings`: **PASS**, warm cache with
  graph changes; not cold or a matched comparison. Detailed measurements are collected separately.
- Cold/warm/UI-edit/backend-edit timings, Release size/startup, matched Tauri/CLI: **PENDING**.
- Runtime responsiveness, cancellation/join and packaged CRT policy: **NOT VERIFIED / HOLD**.
- Overall PERF-02 and migration choice: **PARTIAL**. Do not close #19 or infer migration readiness.
