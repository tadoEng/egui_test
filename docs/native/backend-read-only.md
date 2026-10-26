# Native read-only ext-api adapter (INT-01)

Assessment: **PARTIAL**. This slice adds a native-only optional adapter and
explicit CLI probe, not a frontend migration or a complete native workbench.
Original base: PR #3 head `61f6b74adbbd38255a5736fb0de7f432a3e82cd4`.
The review integration includes current main `aa28552a23da0e9983ed768ba3e1ccaee7937baa`
and its merged WebView #17, preserving both optional features in one lockfile.
Backend source remains read-only at
`9eb1c8792a1d41f3f81c60428afe89f1655782ba`.

## Contracts reused unchanged

| Native boundary | Authoritative source |
|---|---|
| Open existing project | [`AppContext::new_read_only`, `set_sidecar`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/context.rs) |
| Disk-only project status | [`project_status`, `StatusOptions { verbose: false }`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/status.rs) |
| Branches / versions / committed identity | [`list_branches`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/branch.rs), [`list_versions`, `show_version`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/log.rs) |
| Overview | [`get_project_overview`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/overview.rs); only called after authoritative branch/version membership and show_version validation |
| Text version diff | [`diff_versions`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/diff.rs) |
| Supersession | [`SceneLoadGuard`, tickets and status](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/src/model_inspect/packet.rs) |
| Typed error source | [`ExtError`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-error/src/lib.rs); retain the complete anyhow/source chain, never infer categories from English substrings |

The adapter returns source DTOs in small transport wrappers. It does not
recompute version rules, overview verdicts, diff semantics or calculations.
`ext-core` / `ext-db` are transitive dependencies, never an alternative workflow
entry point. `ext-error` is also a direct same-pin dependency for typed matching.
No `ext-tauri` dependency is added.

Ordinary `AppContext::new` performs startup cleanup. The read-only constructor
deliberately avoids it. The adapter also clears any config/env/PATH-resolved
sidecar with `set_sidecar(None)` and never requests verbose/live status. Its
public request enum exposes only Project, Versions (branch history before
selection), Version and Diff reads. There is no
Save/Analyze/Checkout/start-ETABS operation or fixture initializer in that API.

## Worker and identity rules

Two owned worker threads execute reads away from the paint caller. Admission
holds one queued request; the completion queue is also bounded. Submission and
polling never wait for worker completion. Busy admission is a typed outcome.

Every attempted selection claims a generation **before** admission. Even a
busy/rejected project switch invalidates earlier accepted responses. Completion
includes its requested project path, version reference and generation. Source
SceneLoadGuard classifies it; stale payloads are dropped before the caller can
observe them. There is no reuse of a separate guard per project that could
accidentally accept an old project's equal generation.

This service has one active result stream. Chain project/history/version reads;
it does not merge independently submitted concurrent query results. The ticket's
requested reference labels the operation; committed branch/version/hash identity
comes from source DTOs, never from that label.

Dropping the service closes its channels; running disk reads may finish in the
detached worker. This is stale-result suppression, **not cancellation of an
in-progress filesystem/git read**. Explicit join/shutdown, >5 s UI responsiveness,
operation cancellation and packaged lifecycle remain #8 work.

## Run and tests

```powershell
cargo run --locked --offline --features native_backend --example native_backend -- <disposable-project-path>
cargo test --locked --offline --all-features --lib native_backend -- --test-threads=1
cargo clippy --locked --offline --all-targets --all-features -- -D warnings
cargo check --locked --offline --all-targets --all-features --target wasm32-unknown-unknown
```

The probe requires an explicit path and never initializes a project. It currently
exercises project/status reads, not an egui view; version and diff are library
requests covered by tests. Set `RUST_LOG=info` to see its completion generation.
Default builds do not enable native_backend. Native dependencies, module and
example code are target-fenced; the WASM example is inert.

Tests initialize only disposable temporary projects through ext-api, then make
two source-authorized fixture commits with `no_e2k: true, analyze: false` and
sidecar cleared. The placeholder bytes are **not an ETABS database**. Synthetic
content is fixed; source-created timestamps and commit hashes are observed and
compared within each fixture, not asserted to be identical across runs.

The tests compare serialized source DTOs for status, branch/version identity,
details, overview and diff with direct calls to the same pinned ext-api. They
snapshot every fixture file, including .git bytes and modification times, before
and after reads; a partial-snapshot sentinel must survive. Negative cases cover
missing/non-project paths, unknown/traversal references and corrupted state.
Controlled workers force out-of-order completions and busy admission.

The fixture intentionally has no E2K: the authoritative diff is empty with the
source no-E2K warning preserved. Meaningful geometric/E2K differences, scene
packets, checked source provenance and same-source Tauri/CLI end-to-end comparison
remain separate follow-ups. Direct same-source API parity does not pass those
broader criteria.

## Dependency and Windows build evidence

All EtabExtension git records use **one SHA** and Cargo.lock. The original
cross-platform lockfile contains **967 package records**, versus **464** in the
PR #3 baseline: **503 additional records**. Existing baseline name/version
records remain present; this is an integration closure, not a blanket upgrade.
These are lockfile records, not a count of native compiled crates.

[Native normal/build dependency graph with enabled features](evidence/backend-crate-graph.txt).
The root checkout path is replaced with `<workspace>`; the graph contains public
crate names, versions, features and source pins only.

At this pin [ext-api's manifest](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-api/Cargo.toml)
has no lightweight read-only feature split. It unconditionally imports calc,
render and report. The closure includes Polars 0.53, Typst 0.15, Charming SSR,
Deno/V8 and storage/native dependencies. `default-features = false` on ext-api
cannot remove that closure. No upstream crate is edited or replaced to hide it.

The first Windows library check passed and Cargo reported **29m 22s**. It used a
fresh local target with partly populated global caches while other experiment
builds ran. Settings: Rust 1.99.1, `CARGO_BUILD_JOBS=1`,
`CARGO_PROFILE_DEV_DEBUG=0`, `RUST_MIN_STACK=268435456`. Machine: Ryzen 5 6600H,
6 cores/12 threads, 16,312,721,408 bytes RAM, Windows 11 build 26300. This is a
single build observation, not a controlled clean-build benchmark or a comparison
with the current Tauri build, which reuses many of the same backend crates.

The first full test code-generation build reported **63m 40s**, after the check
artifacts had been cached, with `CARGO_PROFILE_TEST_DEBUG=0` as well. Its five
focused adapter tests passed. The final `check.ps1 -Offline` then passed
formatting, all-target/all-feature native check, strict Clippy, **all 28 tests
(23 existing + 5 adapter)**, doc tests and all-target/all-feature WASM check.
These are build/correctness observations, not UI or read-latency benchmarks.

The Windows test link emitted **LNK4098**: default library `libcmt.lib` conflicts
with other libraries. The test executable ran successfully, but that C-runtime
compatibility warning remains unresolved. No `/NODEFAULTLIB`, lint suppression,
upstream patch or global CRT change was used to hide it. Release/packaged
linkage and native dependency/runtime policy require a separate review before
delivery acceptance.

## Review follow-up

The review at `67a9811` correctly identified pending execution evidence. It is
superseded by the completed offline gate at `6336023c37777ee9c98fb086cbe78aad498063f9`,
including the later branch-history request and worker-delivery tests. These
checks were executed and inspected by the implementing parent agent on Windows;
they are not a second independent acceptance review. The fixture's same-source
API comparison does not establish CLI/Tauri end-to-end or engineering parity.

The unconditional dependency closure is a **migration-economics risk**, not a
reason to replace authoritative APIs. A production strategy still requires
matched baseline measurements of Release size, startup, RSS and build cost.
The observations above are not those matched measurements. Combined dependency
graphs and feature checks cannot be inferred from separate green branches.
The current-main integration now includes WebView PR #17 and both
`native_backend` / `webview` features; the exact-head follow-up result belongs
in this PR's review record. Renderer PR #16 remains a separate integration.
No backend feature split, pin change or upstream edit is implied.

Cancellation, explicit worker join and shutdown remain open in
[#8](https://github.com/tadoEng/egui_test/issues/8). No review comment or passing
test closes that lifecycle requirement. INT-01 remains **PARTIAL** and this PR
remains draft pending independent acceptance and explicit owner merge approval.

The original verification JSON and dependency graph describe the pre-main
`6336023` implementation/lockfile. Keep that historical evidence distinct from
the integrated lockfile. Resolving the WebView closure retained the backend pin
and existing runtime code, added its target-specific dependencies and selected
older `toml` 0.8 / `toml_datetime` 0.6 records for cross-platform resolution.
Wry's Linux GTK/glib macro closure pins `toml_datetime = 0.6.3` through
`proc-macro-crate` 2.0.2, conflicting with the previous `toml` 0.8.23 requirement
for 0.6.11. The single resolution selects `toml` 0.8.2, which also participates
in Typst's native Windows graph. This is an actual integration change requiring
the combined checks, not an isolated Linux-only record change. A precise offline
request to retain 0.8.23 reproduced the conflicting constraints. Windows plus
WASM tests do not certify Linux/macOS support.

The dependency-cost/CRT investigation and interface-choice ADR belong in the
separate [PERF-02 #19](https://github.com/tadoEng/egui_test/issues/19) draft. The
cached V8 native archive requests `libcmt.lib`, whereas other MSVC native archives
request `MSVCRT`; the source V8 build script additionally requests `msvcprt` in
the current feature configuration. This establishes a concrete conflicting
directive chain, not a validated allocator/CRT packaging policy. No warning is
suppressed; packaged Release acceptance stays on hold pending compatible native
artifacts and reviewed boundary/lifecycle evidence.

Offline resolution initially lacked a newer non-yanked spin release in the
local index; online locked resolution/cache population resolved that environment
condition without changing the backend pin. Cross-platform manifests also needed
one-time caching before offline WASM checks. No private input was involved.

## Verification ledger

| Criterion | Assessment |
|---|---|
| Single backend source pin and authoritative API boundary | PASS for source/lockfile inspection |
| Windows native all-target/all-feature compile | PASS; final offline gate reproduced |
| Native-only WASM feature fence | PASS for all-target/all-feature WASM check |
| DTO parity and fixture non-mutation | PASS for same-source API fixtures; includes real worker delivery and complete file byte/mtime comparison |
| Typed failures, stale out-of-order and busy-switch suppression | PASS for focused negative tests; classification follows typed causes, not similar prose |
| Final native strict Clippy / all-target checks | PASS |
| Windows C-runtime compatibility | PARTIAL; observed LNK4098 remains unresolved despite passing tests |
| Source Tauri/CLI end-to-end parity and meaningful E2K/geometric diff | NOT TESTED; no-E2K API/reference parity is narrower |
| Release binary size/startup and packaged Windows evidence | NOT MEASURED / NOT TESTED |
| Native GUI, actual slow-job interaction, cancellation and teardown | NOT TESTED |
| Scene/provenance, numeric calculations, charts, reports and sidecar runtime | Deferred to the separate gate/integration issues |

No engineering parity or migration readiness is declared from compilation. No
production changes, private-derived artifacts or default ETABS launch.
