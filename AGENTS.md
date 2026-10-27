# Local Rust verification tools

Follow the owner's requested worktree and scope. Do not edit production EtabExtension while working in this feasibility repository.

- Use installed **sccache** and **cargo-nextest** for the owner-approved BUILD-06 step8 workflow. Setup and verified limits: [docs/native/build-tools.md](docs/native/build-tools.md).
- Keep the project toolchain pinned by `rust-toolchain` (currently1.99.1). Rust1.96.0 applies to the separate EtabExtension BUILD-06 work/tool-install commands; do not downgrade this project to install a tool.
- Give every worktree its own `.codex-target`. Never borrow/share another worktree's target directory. Sharing compiler-cache entries is separate from sharing target artifacts.
- Prefer `--target-dir .codex-target`. For the unchanged `check.ps1`, run from that worktree with process-local `CARGO_TARGET_DIR=.codex-target`. Avoid a different absolute CARGO_TARGET_DIR per worktree: this defeated dependency reuse in the pinned sccache probe.
- Set `RUSTC_WRAPPER` to the installed sccache executable only in the owned command/session. Put its owned compiler cache and temporary files on D:. Verify the server's reported cache directory; Cargo home's junction alone does not relocate either.
- Preserve shared cache servers/counters. An owned server needs a separate cache/port and verified listener/PID/creation-time ownership before cleanup. Keep reusable compiler entries; remove only disposable owned probe files.
- Plain `cargo test` and the canonical `check.ps1 -Offline` remain supported and mandatory. Nextest is an additional pilot runner until full-project test-name/count parity and gate policy are independently accepted; it does not replace doc tests, GPU probes, fmt, Clippy or WASM checks.
- Use jobs1 / nextest test-threads1 while memory is constrained. Preserve existing memory/disk guards; do not claim a speedup from successful installation or a tiny functional probe. Record tool/cache/target settings for new measurements and leave historical receipts unchanged.
- Only these two tools are authorized for installation here. Working installed binaries need no reinstall/download. Do not change global Cargo profiles, linker/CRT/security settings or production gates as part of this setup.
