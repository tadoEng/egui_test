# APP-01C Windows correction and verification follow-up

This branch reuses draft PR #56 at f863ca8d774946d6ad2e76e6cd6b2067310941ee, based on main de080d03. The existing ECharts page, theme, typed bridge, stored lab-results values and bounded offline child host remain the implementation. The earlier local attempt is preserved separately and is not included.

## Blocking defects corrected in source

- Use eframe 0.37 CreationContext directly as HasWindowHandle, as the original PR #56 does. The handoff audit corrected an erroneous winit_window-field adaptation introduced in the first follow-up commit.
- Remove the silent native-chart fallback. Creation, bootstrap, script, refused-navigation and placement errors remain explicit in the shell. A failed child is dropped, so a failed hide cannot leave it over Model. The stored table remains readable.
- Apply native actions before consuming queued chart messages. Expire the old frame on query/version/workspace changes, scope forwarded shortcuts, and hide the child immediately after navigation to Model.
- Record child-owned focus before hiding, then restore the native parent without stealing focus from another application.
- Retain generation changes and selection-only updates through the existing bridge. Historical Linux/browser receipts are unchanged; old unscoped shortcut samples are now expected to be refused.

## Current assessment: BLOCKED - Windows execution pending

Only source formatting, locked offline metadata and whitespace validation have run on this follow-up. Tests are authored, not passed. The prior Linux receipts belong to PR #56 and are not Windows proof for this branch.

The owner will execute the Windows checks on a remote machine; local verification and the follow-up are paused. The owner permits targeted local checks at 5 GiB available RAM, with the 2 GiB runtime floor retained. No APP-01C Cargo build, TALOS launch, compiler-cache server or GUI automation has been started. Root .codex-target is preserved; no cargo clean, downloads or global build changes.

Next: targeted Windows compile, plain tests and strict Clippy; canonical offline/doc/WASM checks and additional nextest; then an exact-binary public-fixture WebView2 smoke covering stored values, chart/table selection, stale refusal, resize, focus and Model/Results visibility/hit testing. Physical mouse, DPI and compositor behavior require actual Windows observations and must not be inferred from headless or scripted input.

Viability is undecided until these checks execute. No migration, production EtabExtension change, private evidence, APP-01D or merge is authorized by this follow-up.
