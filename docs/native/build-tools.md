# Owner-approved sccache and cargo-nextest setup

This is the two-tool portion of [EtabExtension BUILD-06 step8](https://github.com/tadoEng/EtabExtension/issues/466). No other step/tool/download is implemented here. Production EtabExtension is unchanged. `check.ps1`, Cargo manifests/locks, profiles, linkers and application sources are unchanged.

Both tools already work on the owner machine: sccache0.19.0 and cargo-nextest0.9.150. No installation/download was needed. Cargo home resolves through a junction to D:, but nextest currently resolves from the existing WinGet Links directory and sccache's default cache reports C:. Default TEMP also reports C:. Do not assume that Cargo home's location alone keeps build/cache/temp writes on D:.

If a future machine lacks a tool, the owner-authorized source-install commands are:

```powershell
cargo +1.96.0 install --locked sccache
cargo +1.96.0 install --locked cargo-nextest
sccache --version
cargo nextest --version
```

Do not rerun installs merely to change an already working binary's installation origin. This egui project's `rust-toolchain` remains1.99.1; the functional tool probes used1.96.0 without modifying that pin.

## Worktree use

First provision/use a verified **owned D:-backed cache server** and set its SCCACHE_SERVER_PORT. The probe used a fresh loopback port and separate cache directory, verified the reported directory plus listener/PID/start time, and stopped only that server. Do not stop/reset an existing shared server or silently redirect it. Set SCCACHE_DIR before launching the owned server; setting it on a client does not reconfigure an already running daemon. Keep useful compiler-cache entries between real worktree runs.

Within the intended worktree, use session-local settings. Supply existing owned D: directories for SCCACHE_DIR/TEMP/TMP and set RUSTC_WRAPPER to the installed executable. Do not change machine/user environment settings.

```powershell
$env:RUSTC_WRAPPER = (Get-Command sccache).Source
$env:CARGO_BUILD_JOBS = '1'
$env:RUST_MIN_STACK = '268435456'

# Explicit target selection: separate .codex-target in each worktree.
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
cargo test --workspace --all-targets --all-features --locked --offline --target-dir .codex-target
cargo nextest list --workspace --all-targets --all-features --locked --offline --target-dir .codex-target --message-format json
cargo nextest run --workspace --all-targets --all-features --locked --offline --target-dir .codex-target --test-threads 1
cargo test --workspace --doc --locked --offline --target-dir .codex-target

# The existing canonical script has no --target-dir argument:
# use the SAME RELATIVE value from the intended worktree.
$env:CARGO_TARGET_DIR = '.codex-target'
./check.ps1 -Offline
```

Nextest is currently an **additional pilot**, not a replacement for mandatory checks. Compare selected names/counts and ignored cases before adopting it across the actual workspace. Keep doc tests and explicitly sanctioned GPU/oracle probes separate. No parallelism/profile/linker change is adopted by this setup; test-threads1 protects current headroom. A more parallel policy needs its own measurements and approval.

## Actual functional verification

Two independently located disposable Cargo projects had identical public source and separate `.codex-target` directories. Only a cached registry dependency (`itoa1.0.18`) was used; all Cargo commands ran offline with1.96.0. Each project had two tests, checking42 and-7. In each directory plain Cargo and nextest passed2/2, with identical discovered names and no skips. The library tests were not incrementally cacheable; test/link work was not advertised as cached.

Three configurations were observed with fresh isolated servers/caches:

| Target selection | Registry Rust misses after A/B | Registry Rust hits after A/B | Result |
| --- | --- | --- | --- |
| Different absolute CARGO_TARGET_DIR per source directory |1 /2|0 /0|Functional PASS; no cross-directory reuse |
| Per-project `--target-dir`, absolute target env unset |1 /1|0 /1|Functional PASS; dependency reuse |
| Same relative `.codex-target` env, cwd set to each project |1 /1|0 /1|Functional PASS; canonical-script-compatible reuse |

All configurations used a shared **owned compiler cache**, never a shared target. Incremental workspace calls remained noncacheable (4 after both directories). No path normalization/remapping, incremental profile, global configuration or linker flags were changed. The pinned sccache source hashes all CARGO_ variables except specific exclusions; different absolute target env values account for the missed reuse. Its documentation also excludes incremental and link-producing Rust compilations: [Rust caching](https://github.com/mozilla/sccache/blob/v0.19.0/docs/Rust.md), [source hashing](https://github.com/mozilla/sccache/blob/v0.19.0/src/compiler/rust.rs).

This is a **functionality/cache-isolation proof**, not an EtabExtension/egui full-gate benchmark, parallel test acceptance or a quantified speedup. No source models, private artifacts or production tests were used. Raw local logs remain outside Git; [sanitized test/cache/cleanup receipt](evidence/build-tools/windows-validation.json) contains only public probe facts and log hashes. Owned probe directories/caches and verified owned servers were removed/stopped after evidence capture; shared resources were left alone.

Week1 priority remains #35 → #39/#40 → separately reviewed #41 → version-scoped model/results integration. This documentation adds no product feature or migration acceptance.
