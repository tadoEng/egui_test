# Public smoke completion and report ownership

Refs #28 / #8. Source audit of main `1b9aab7bfc4c03b3052249d2ca97abbc5f2ee976` found two defects in `native_dashboard` and `webview_stage_a`: a failed report/window close could still return exit0, and `std::fs::write` could truncate an existing report. Existing external gates already required fresh JSON PASS plus exit0; their historical accepted PASS receipts are preserved at their original sources.

Both examples now use std-only `examples/support/smoke_receipt.rs`. An explicit smoke report path is reserved **before native/WebView creation** with `create_new`; the original handle is retained through completion. Existing files/directories, denied access and missing parents fail before window startup. No parent directories are created and no existing file is truncated/reopened/deleted. Windows permits readers and refuses cooperative writers/deletion while the handle is owned. This is cooperative ownership, not hostile filesystem/reparse certification.

A bounded channel carries the completed callback verdict through window teardown. Success requires passing bridge/callback checks and completed report write/flush. FAIL, timeout, missing/early-close verdict or I/O failure returns a main error/nonzero exit. Host errors cannot coexist with PASS. Visible mode and chart/fixture data remain unchanged; no report is created without its explicit environment variable.

## Public controls and acceptance

Use fresh owned temporary paths via `DASHBOARD_SMOKE_REPORT` or `WEBVIEW_SMOKE_REPORT`, set only on the owned child process. Optional `TALOS_PUBLIC_SMOKE_CONTROL` is unset for normal, `fail` for forced failure, or `missing-receipt` to close without completing output. Controls require report mode and reject unknown values. Both examples remain authored-synthetic only; no private/project input is introduced.

| Probe | Required actual outcome |
| --- | --- |
| Normal | Fresh JSON PASS, no host error, actual bridge/snapshot/callback invariants and self-exit0 |
| `fail` | JSON FAIL, `controlled_failure:true`, self-exit nonzero |
| `missing-receipt` | Empty reserved file, specific missing-receipt stderr reason, self-exit nonzero |
| Existing authored sentinel | Nonzero before native startup; bytes and mtime unchanged |
| Missing parent / invalid target | Nonzero before startup, no directory creation or unrelated writes |
| Unknown control / control without report | Nonzero before startup, no visible-mode override |

Require a current exact-source build/artifact hash. An old binary ignoring a control or a timeout/externally killed process is not proof. Preserve sanitized receipts before cleaning only owned disposable test files. No new launcher or profile policy is introduced; parent exit/held file cleanup does not prove browser subprocess shutdown. Do not touch owner previews, shared WebViews or caches, and do not fabricate physical input/network/presentation proof.

## Verification

Formatting, locked offline no-dependency metadata and whitespace are source-only checks. Six distinct shared Rust controls (compiled by both Windows examples) cover existing bytes/mtime preservation, fresh completion, failed/missing channel verdicts, absent-parent refusal, failed flush and explicit negative-mode parsing. **Rust tests/native probes are written but not executed yet.** Their full canonical/native/WASM/strict-Clippy/plain-Cargo/doc gate and additional installed-nextest single-thread pilot must run serially after higher-priority #41 validation; no test count/PASS is claimed prematurely.

No dependency/lockfile/backend pin/profile/linker/CRT/gate change. Keep Rust1.99.1, installed sccache/nextest only, worktree-local D:.codex-target, owned D: cache/temp/server, jobs1/stack268435456 and existing launch/runtime resource guards. No downloads, borrowed targets or cache deletion. This small main-based fix remains separate from #41/#44 acceptance, physical Windows/lifecycle/engineering fidelity and packaging. Tauri remains production and all six migration gates stay open.
