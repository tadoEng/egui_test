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

The integrated lockfile has 1056 cross-platform records, versus the pre-WebView backend's 967
and current main's 595. The old 464-record comparison describes the pre-WebView baseline.
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
the threshold. For the upcoming Release phase, it additionally enforces a 2 GiB system-memory
headroom guard at launch and during sampling, with a distinct stopped-for-memory outcome.
It never closes user applications, deletes caches or follows target reparse points when counting bytes.
The completed compiler-check phase used the prior harness without that added guard; both
Release configurations use the revised harness and record its hash/resource thresholds.
It requires **PowerShell 7+**; Windows PowerShell 5.1 is unsupported. Receipts record
PowerShell/.NET, Cargo/Rust and non-identifying Windows CPU/architecture/RAM inventory.
Controlled runs reject all tracked edits and non-ignored untracked files, plus ignored files
in compiled-input roots (`src`, `examples`, `tests`, `benches`, `assets`, `.cargo` and root
Cargo/build/toolchain files). Edit measurements use explicit disposable commits. HEAD, Git
tree and lockfile hashes identify the inputs; final clean status, HEAD and lockfile are checked
again. This does not claim hermeticity over populated global registry/OS caches.
Stdout/stderr are streamed to local files and hashed after exit; raw logs and local paths are
not automatically published. OS process start/exit duration is separate from the runner's
sampling overhead, avoiding one-second rounding of a fast no-op run.
Raw local reports/logs require sanitization before publication. Fast no-op runs can exit before
memory sampling; unavailable memory is null rather than a fabricated zero.

Use disposable benchmark checkouts for UI/backend edit cases; never edit a reviewed PR to create
a benchmark. Run one heavyweight compilation at a time. Use fresh uniquely named target folders
on the same drive/settings for cold comparison; do not use `cargo clean` on shared targets.
At least two no-op/edit repetitions are feasible; cold repetition depends on disk/time. Record
single runs honestly. Startup must name its readiness event; process launch or cargo completion
alone is not an interactive-host readiness measurement.

## Reproduce the integrated source

The measured integration is now published on draft
[#18](https://github.com/tadoEng/egui_test/pull/18), retargeted to main. It is an experimental
review candidate, **not production merge approval**. GitHub retrieval of the immutable
[commit](https://github.com/tadoEng/egui_test/commit/cf4fbb218da1e62cf767a17a35e49f9c4f48e50e),
[manifest](https://github.com/tadoEng/egui_test/blob/cf4fbb218da1e62cf767a17a35e49f9c4f48e50e/Cargo.toml)
and [lockfile](https://github.com/tadoEng/egui_test/blob/cf4fbb218da1e62cf767a17a35e49f9c4f48e50e/Cargo.lock)
provides the source missing at the first #21 review. The economics PR deliberately does not
copy that runtime adapter. Use a disposable checkout and the measurement runner from this PR:

```powershell
git fetch origin codex/native-read-only-api
git worktree add --detach <new-disposable-checkout> cf4fbb218da1e62cf767a17a35e49f9c4f48e50e
./scripts/measure-cargo.ps1 -Checkout <new-disposable-checkout> -TargetDirectory <new-empty-target> `
  -ReportDirectory <local-raw-reports> -Label backend-cold-check -RequireEmptyTarget `
  -CargoArguments @('check','--locked','--offline','--all-targets','--all-features','--timings')
```

Offline replay requires a populated package cache. The original 93.7 s receipt records the
historical harness at `6b26fca`; it includes its polling overhead and remains a warm graph-change
observation. Revised runner semantics do not retroactively convert it into a controlled cold run.

## Evidence ledger

### Controlled check campaign

The baseline phase completed on the frozen main/WebView source, before starting the matching
backend phase. All commands use `check --locked --offline --all-targets --all-features --timings`
and the same one-job/no-debug settings above, with separate fresh targets on C:. Only one owned
build runs at a time. These are compiler checks, not executable builds or application startup.

| Baseline condition | Cargo process duration | Source / evidence |
|---|---:|---|
| Fresh target, one run | 438.132 s | [`aa28552` receipt](evidence/perf19/baseline-cold-check-1.json), [Cargo units](evidence/perf19/baseline-cold-check-1.units.csv) |
| No-op, two runs | 0.752 / 0.708 s | [run 1](evidence/perf19/baseline-noop-check-1.json), [run 2](evidence/perf19/baseline-noop-check-2.json) |
| One egui caption edit per committed run | 2.300 / 2.186 s | [run 1](evidence/perf19/baseline-ui-check-1.json), [run 2](evidence/perf19/baseline-ui-check-2.json) |

The cold target grew from zero to 652,647,457 logical file bytes; its sampled peak sum of
working sets was 1,732,202,496 bytes. This includes shared-page double counting and may miss
short-lived processes; it is not an exact private RSS peak. Raw log hashes and input identity
checks passed before sanitizing report paths. Global registry/source and OS caches were populated;
user processes were left running. One fresh-target run is not a statistical capacity claim.

Edit probes are reachable on the **measurement-only, not-for-merge** branch
`codex/perf19-baseline-probe`: [first caption edit](https://github.com/tadoEng/egui_test/commit/1988f0cb5889c65f9a8f3cff5f57380dbc8e2edc)
and [second caption edit](https://github.com/tadoEng/egui_test/commit/df35ba9799a4a41116dec8bc6abc866173975ce0).
Only the single Model Explorer heading changes. Reviewed implementation branches are untouched.

The matching backend phase completed with the same command and settings. Its all-target scope
also includes the adapter's additional CLI example, as declared by that configuration.

| Condition | Main + WebView | Main + WebView + read-only API |
|---|---:|---:|
| Fresh target, one run per configuration | 438.132 s | [1323.492 s](evidence/perf19/backend-cold-check-1.json) |
| No-op, two runs | 0.752 / 0.708 s | [1.581](evidence/perf19/backend-noop-check-1.json) / [1.497 s](evidence/perf19/backend-noop-check-2.json) |
| One egui heading edit, two committed runs | 2.300 / 2.186 s | [4.305](evidence/perf19/backend-ui-check-1.json) / [4.275 s](evidence/perf19/backend-ui-check-2.json) |
| One lab adapter diagnostic-text edit, two committed runs | No adapter | [4.198](evidence/perf19/backend-adapter-check-1.json) / [4.152 s](evidence/perf19/backend-adapter-check-2.json) |
| Cold target logical file bytes | 652,647,457 | 2,017,834,545 |
| Sampled cold peak sum of working sets | 1,732,202,496 bytes | 2,813,399,040 bytes |

The cold check was **3.02 times longer in this one paired observation**. That is not a
statistical/general ratio, a Release build comparison or a production Tauri comparison.
Cached UI checks took seconds in both configurations; this supports retaining the existing
feature fence for lab development, but says nothing yet about executable relinking or startup.

The [backend Cargo units](evidence/perf19/backend-cold-check-1.units.csv) contain 943 unit
records versus the baseline's 321; repeated host/target/build-script units are not unique crates.
The largest individual recorded unit is `deno_ops` at 55.38 s, followed by macro/bindgen units.
The cost is spread across the dependency closure; it is not attributed to one guessed bottleneck.
Per-run CSVs export every selected name/version/target/mode/start/duration row from Cargo's
timing data. Row counts and floating-point values were round-tripped before publication; raw
HTML/JSON remain local. Receipts keep original log hashes. Checkout raw lockfile SHA-256 may
differ with Git LF/CRLF conversion; Git commit/tree identity and before/after raw hashes are
recorded separately, with no lock semantics changed during measurement.

Backend edit source is reachable on the **measurement-only, not-for-merge** branch
`codex/perf19-backend-probe`, ending at
[`c5a96e2`](https://github.com/tadoEng/egui_test/commit/c5a96e26ea0206efff93f7408885017fda24bd49).
The first two commits change the same egui heading; the next two append diagnostic display text
in the lab adapter. No authoritative `ext-api` source or domain rule was edited. This does not
measure rebuilding an upstream backend crate. Reviewed draft implementation branches stay unchanged.

### Paired Release build and relink phase

Both configurations build the same GUI binary and WebView example with
`build --release --locked --offline --all-features --bin eframe_template
--example webview_stage_a --timings`, on separate fresh C: targets. This two-target
lab workflow includes example dev-dependencies; it is not a minimal shipping-package build.
The baseline uses the original `aa28552` source and Release harness `4b64083` with the recorded
one-job/opt-level-2/no-debug/no-LTO/no-incremental profile and memory/disk guards.

| Baseline Release condition | Cargo process duration | Evidence |
|---|---:|---|
| Fresh target, one run | 686.652 s | [receipt](evidence/perf19/baseline-cold-release-1.json), [units](evidence/perf19/baseline-cold-release-1.units.csv) |
| No-op, two runs | 0.841 / 0.766 s | [run 1](evidence/perf19/baseline-noop-release-1.json), [run 2](evidence/perf19/baseline-noop-release-2.json) |
| Committed widget edit, rebuild and link | 17.421 / 17.058 s | [run 1](evidence/perf19/baseline-ui-release-1.json), [run 2](evidence/perf19/baseline-ui-release-2.json) |

The baseline GUI EXE is **17,955,840 bytes** and WebView example **18,573,824 bytes** in the
[cold artifact manifest](evidence/perf19/baseline-cold-release-1.artifacts.json). Each run has
its own source/binary hashes; widget edits can change a hash without changing file size.
The cold Release target contains 1,475,051,408 logical file bytes; sampled peak summed working
sets are 1,837,096,960 bytes, with minimum observed available system memory 4,893,945,856 bytes.
Neither resource guard stopped the run. Logs, identity and artifact hashes were verified.

These executable rebuild results are distinct from the earlier 2.2 s compiler-only UI checks.
Release UI edit commits are reachable on the measurement-only
`codex/perf19-baseline-release-probe` branch:
[`e033bdda`](https://github.com/tadoEng/egui_test/commit/e033bddac2641fb8541668a9db07e07cad281a40)
and [`014f576c`](https://github.com/tadoEng/egui_test/commit/014f576c3202df386b6b5b937b74a7cf0d163b40).
No structural GUI was launched against persisted user state. The API-enabled phase now
completes the same two-target command, profile and guard policy at the frozen `cf4fbb2` source.
The comparison remains against `aa28552`, **not** the newer renderer merge `edab698` on main.

| Release condition | Frozen baseline | API enabled |
|---|---:|---:|
| Fresh target, one run | 686.652 s | [3597.064 s](evidence/perf19/backend-cold-release-1.json) |
| No-op, two runs | 0.841 / 0.766 s | [1.997](evidence/perf19/backend-noop-release-1.json) / [1.663 s](evidence/perf19/backend-noop-release-2.json) |
| One widget-caption edit, actual build/relink | 17.421 / 17.058 s | [32.841](evidence/perf19/backend-ui-release-matched-1.json) / [32.475 s](evidence/perf19/backend-ui-release-matched-2.json) |
| GUI executable bytes | 17,955,840 | 45,646,336 |
| WebView example executable bytes | 18,573,824 | 19,971,072 |
| Cold target logical file bytes | 1,475,051,408 | 5,075,178,197 |
| Sampled cold peak summed working set, bytes | 1,837,096,960 | 2,994,036,736 |
| Minimum sampled available system memory, bytes | 4,893,945,856 | 3,723,042,816 |

The single cold Release observation was **5.24×** the baseline duration (about 60 versus
11.4 minutes). Two matching caption edits used the same literal strings as the baseline.
An additional earlier, differently worded caption probe measured
[32.501 s](evidence/perf19/backend-ui-release-1.json); it is retained as **exploratory**,
not substituted into the matching pair. All three are independent nonincremental root
rebuilds on the same warm target; this extra observation is not a third cold repetition.
No retry or failed run is hidden. These timings do not measure UI interaction, GPU frames,
API project readiness, startup or a production Tauri comparison.

Release edit sources are on **measurement-only, not-for-merge**
`codex/perf19-backend-release-probe`: exploratory
[`54ddd4c`](https://github.com/tadoEng/egui_test/commit/54ddd4c6ef73b105955b4c906fa2af5a55c6e25c),
matching [`8e4f80b`](https://github.com/tadoEng/egui_test/commit/8e4f80b933cf793994948729941f339eedf26f08)
and [`7229518`](https://github.com/tadoEng/egui_test/commit/7229518bd07399c939eb46a5f06705c5a5460c72).
Each run includes a source/binary hash manifest, verified raw-log hashes and selected Cargo
units CSV. The cold API build recorded 942 timing units, not 942 unique crates. Global
registry/source/OS/native artifact caches were populated; user applications remained open.
This is one serial Windows machine campaign, not a statistical or cache-wiped comparison.
Neither resource guard stopped a run. Summed working sets may double-count shared pages and
miss short children; they are not a precise RSS or packaged runtime-memory measurement.

**CRT disposition: HOLD.** The cold API GUI executable link and actual caption rebuilds
emit `LNK4098: defaultlib 'libcmt.lib' conflicts with use of other libs`. This now reproduces
in an actual optimized executable, beyond prior debug/check evidence. No-op builds may replay
cached warnings and are not independent link attempts. No warning was suppressed, no global
CRT flag changed, and no upstream source modified. Successful exit and produced EXEs do not
establish allocation/runtime safety or a safe package. The WebView example's smaller size is
not proof of complete API linkage or project-workflow readiness. Keep #18 on packaging HOLD;
defined startup/readiness and clean worker shutdown remain unverified.

- Dependency roots and CRT directive chain: **PASS for inspected source/artifacts**.
- Integrated first `check --all-targets --all-features --timings`: **PASS**, warm cache with
  graph changes; not cold or a matched comparison. Detailed measurements are collected separately.
- Paired cold-target/no-op/UI-edit and lab-adapter-edit compiler checks: **PASS for the recorded runs**;
  one cold pair and two warm/edit repetitions, not a general performance guarantee.
- Paired Release cold/no-op/widget-rebuild and binary-size observations: **PASS for recorded runs**;
  optimized API executable still emits LNK4098, so package safety is not accepted.
- Startup/readiness and matched Tauri/CLI: **PENDING**.
- Runtime responsiveness, cancellation/join and packaged CRT policy: **NOT VERIFIED / HOLD**.
- Overall PERF-02 and migration choice: **PARTIAL**. Do not close #19 or infer migration readiness.
