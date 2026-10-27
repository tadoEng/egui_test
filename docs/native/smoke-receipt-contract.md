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

Formatting, locked offline no-dependency metadata and whitespace are source-only checks. Six distinct shared Rust controls (compiled by both Windows examples) cover existing bytes/mtime preservation, fresh completion, failed/missing channel verdicts, absent-parent refusal, failed flush and explicit negative-mode parsing. **Actual execution passes at source `66ac44e133e300229516368aca728940c1e5aa7b`.** `check.ps1 -Offline -SkipWeb` passes fmt, native all-target/all-feature checking, strict Clippy,124 plain-Cargo CPU executions (including shared repeats;2GPU instances ignored), and3empty doc-test suites. The identical mandatory WASM command passes separately with process-local `RUSTC_WRAPPER` unset because the preceding #41 validation demonstrated sccache Windows spawn error206 for `web-sys`. Native commands still use installed sccache; fallback warnings are preserved. This is an explicitly split native/WASM gate, with no profile/toolchain/CRT change or cache-speed claim. Nextest additionally passes124/2skipped with126listed; test-name and ignored-name multisets match plain Cargo. The explicit public depth GPU probe passes on RTX3050Ti/Vulkan591.74 and its image was inspected.

Both exact-current Debug/debug0 executables were built and tested, rather than reusing #17/#31 artifacts. Sixteen actual process cases pass (eight per executable): normal JSON PASS + exit0; controlled JSON FAIL + exit1; empty reserved missing-receipt file + specific error + exit1; existing sentinel bytes/mtime preserved + refusal; missing parent left absent + refusal; control without report refused; unknown control refused; and an effective owned-directory CreateFiles-denied case refused. Original ACLs were restored and owned fixtures removed. Every root self-exited; timeout/kill is not accepted as an expected failure. Positive and forced reports also retain the expected real programmatic bridge snapshots/selection invariants and false physical-input/network/presentation flags.

[Source, command/log/resource receipts](evidence/smoke-receipt-contract/windows-validation.json), [16-case native matrix and exact artifact hashes](evidence/smoke-receipt-contract/native-controls.json), [nextest name parity](evidence/smoke-receipt-contract/nextest-parity.json). Local verification driver identity is recorded; raw logs/paths stay local. Final evidence changes no compiled input. **PASS for this smoke-contract correction; overall programme PARTIAL pending independent re-review.** This does not certify WebView subprocess cleanup, worker join or physical Windows UX. No shared/owner application was stopped and useful compiler caches/logs remain intact.

No dependency/lockfile/backend pin/profile/linker/CRT/gate change. Keep Rust1.99.1, installed sccache/nextest only, worktree-local D:.codex-target, owned D: cache/temp/server, jobs1/stack268435456 and existing launch/runtime resource guards. No downloads, borrowed targets or cache deletion. This small main-based fix remains separate from #41/#44 acceptance, physical Windows/lifecycle/engineering fidelity and packaging. Tauri remains production and all six migration gates stay open.
