# WB-02-A CPU certification replay notes

The measured source is `dfdb302666f2a80d3ac678a6475b8bcd1c6eeaa7`. Check out that commit in a clean physical worktree with the pinned toolchain and installed tools. Keep each worktree's own `.codex-target`. Never share a target or stop another cache server. Read the repository AGENTS.md and build-tools policy first.

The archived `invoke-guarded.ps1`, `cargo-step.ps1`, `stop-owned-sccache.ps1` and `summarize-parity.ps1` are byte-exact. `run-p0.ps1` and `start-owned-cache.ps1` replace only machine-local path bindings with parameters; they were not literally executed in this form. The validation JSON records original and published SHA256 plus a mechanical reverse-normalization comparison. No other behavior cleanup was applied.

From PowerShell7, supply your clean checkout, a writable owned D: root, and a fresh run directory beneath that owned root:

```powershell
& ./harness/run-p0.ps1 -CheckoutRoot $checkout -OwnedRootPath $ownedRoot -RunDirectory $freshRun
& ./harness/summarize-parity.ps1 -RunDirectory $freshRun
```

The launcher chooses an unused loopback port, verifies listener/PID/start time and reported cache directory, then serializes every command. Each launch checks6GiB available RAM/disk. Runtime checks require2GiB RAM/6GiB disk; sampling errors stop the owned process tree. It never closes owner apps or deletes caches. Its finally block verifies the current owned replacement listener before stopping it. Run under the approved Windows CIM access context; denial is a sampling failure, not a fabricated memory measurement.

Commands, outcomes, source/lock/log/harness hashes, approximate resource samples and normalized path-binding provenance are in `windows-validation.json`. Native canonical used `-SkipWeb`; the mandatory identical WASM check ran separately with the child wrapper unset for the documented Windows sccache os206 condition. Nextest is additional parity, not a replacement for plain Cargo/docs. Raw logs and machine-local paths are deliberately omitted.

`manifest.csv` is deterministic authored synthetic data. `cpu-budget.csv` contains three serial samples after one warmup per preset in one Release process. No GPU smoke, upload time, presentation percentile, FPS, driver-memory or migration acceptance is asserted. CPU logical stream bytes are not a process-memory limit.
