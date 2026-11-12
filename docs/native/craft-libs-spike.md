# Craft libraries compatibility spike — no-patch (2026-10-28)

**Status: draft, experimental `egui_test` only. Decision input, not an
adoption.** Requested in [#49](https://github.com/tadoEng/egui_test/issues/49)
(owner decision, 2026-10-28). **This branch changes no dependency:**
`Cargo.toml` and `Cargo.lock` are identical to `main`. The probes ran in scratch
directories and only their logs are committed.

| Item | Value |
|---|---|
| craft-libs | `storytold/craft-libs` @ `b4afa6d22ae41bd36524c32dbcab3ff257ae4cdd` (MIT OR Apache-2.0; `NOTICE`, `ATTRIBUTION.md`) |
| egui_test base | `main` @ `a81695baaad8622b2c8437abe6f08936feca8f4a` |
| Toolchain | Rust 1.99.1; published egui/eframe 0.37.0; no `[patch]` anywhere |
| Scope | `craft-ui` (`tabs`, `panes`) only, `default-features = false` |

## Results

### 1. Against the existing `egui_test` lockfile: refused at resolution

The probe added `craft-ui = { git, rev = b4afa6d, default-features = false }`
as a workspace member. `cargo check -p craft-ui-probe` **fails before
compiling** (`evidence/craft-spike/resolve-against-existing-lock.log`):

```text
failed to select a version for `icu_locale_core`
  craft-i18n (dep of craft-ui) requires icu_locale_core = "~2.3"
  egui_test's Cargo.lock has icu_locale_core 2.4.0 (url → idna → idna_adapter → icu_properties 2.4)
```

`craft-i18n` pins the whole `icu_* ~2.3` family. Cargo cannot hold two
semver-compatible `2.x` versions, so adoption would require **downgrading
production lock entries** that `url`/`idna` resolve today. That was not done:
it would paper over the incompatibility with a dependency change.

### 2. Isolated fresh workspace, published egui 0.37, no patch: does not compile

A standalone scratch workspace (fresh lockfile, 126 packages) resolves:
`craft-ui → craft-fonts-build, craft-i18n, egui 0.37.0, epaint 0.37.0, log,
skrifa 0.44.0` (`skrifa` matches the version `egui_test` already locks).
Compilation of `craft-ui` **fails with 5 errors**, all on Storytold egui-fork
APIs that published egui 0.37 does not have
(`evidence/craft-spike/isolated-no-patch-check.log`):

| Fork-only symbol | Used in |
|---|---|
| `egui::ime_context::{ImeContextTracker, ImeObservation, ImeTarget}` | `crates/ui/src/ime/composer.rs:5` |
| `egui::IMEOutput::new(owner, generation, purpose, rect, rect)` | `crates/ui/src/ime/composer.rs:528` |
| `IMEOutput.owner` | `crates/ui/src/ime/window.rs:69` |
| `egui::Style::right_to_left` | `crates/ui/src/combo.rs:289`, `:395` |

Neither `tabs/` nor `panes/` names these symbols. But `craft-ui` is one crate
with no per-module feature gates, so **neither can be taken as a dependency
without the fork**. Rustc reports the errors it reaches, so other fork-only
uses may be masked behind these five.

### 3. `tabs` module alone: compiles on published egui

`crates/ui/src/tabs/mod.rs` imports only `egui`. Copied into a **scratch-only**
crate (test module tail removed) and checked offline against `egui_test`'s
existing lockfile, it **compiles with no errors or warnings**
(`evidence/craft-spike/tabs-module-only.txt`). Nothing from it is committed.

`panes/` depends on `craft-ui` internals (`docking::chrome`, `layout`, `focus`,
`docking::PanelLimits`) and pulls in a much larger surface. It was **not**
extracted.

## ADOPT / ADAPT / DEFER

| Item | Decision | Why |
|---|---|---|
| `craft-ui` as a dependency (any module) | **DEFER / do not adopt** | Requires the Storytold egui fork (result 2), and its `craft-i18n` pins conflict with the existing lock (result 1). |
| `craft-ui::tabs` design: `fit` / `fit_whole` (shrink-then-overflow, selected tab always retained) and caller-painted `Tab::show_at` | **ADAPT (pattern)** | Compatible in isolation (result 3), and its model matches APP-01A: the app owns identity and selection, and the strip only assigns widths and reports interactions. If wanted, write a small internal implementation behind `talos-native`'s `tabs.rs`, crediting craft-libs (MIT OR Apache-2.0) in the file header and keeping that crate's license terms. Needed only once the strip can overflow (narrow windows or more workspaces). |
| `craft-ui::panes` (PaneHost, edges, rail, F6 cycling) | **DEFER** | Coupled to `docking::chrome` and `layout`. APP-01A's plain `egui::Panel` sides suffice for optional explorer/inspector. Revisit after the Windows WebView2 z-order/focus tests (APP-01C). |
| `craft-ui::docking` | **DEFER** | Per owner decision; WebView2 child windows are not egui widgets. |
| `craft-keymap` | **DEFER** | Later, with a command palette. Not compiled here. |
| `craft-settings` | **DEFER** | Separate storage-policy review. |
| `craft-pdf`, `craft-ai-core`, `craft-atomic`, fonts, assets, brand | **NOT ADOPTED** | Out of scope; reporting stays on `ext-report`. |

Licensing: craft-libs library code is MIT OR Apache-2.0 (`LICENSE-MIT`,
`LICENSE-APACHE`). Its `NOTICE` and `ATTRIBUTION.md` list bundled third-party
assets (ICC profiles, ICU collation data under Unicode-3.0, hyphenation patterns
including one GPL-2.0-or-later language, OFL test fonts). None of those are in
the `tabs` module. Any adapted code must carry the MIT/Apache notice, and no
asset may be taken without its own license.

## What would change this

- Upstream egui gains the IME-context and right-to-left APIs, or craft-ui gates
  `ime` and `combo` behind features. Then re-run result 2.
- `craft-i18n` relaxes `icu_* ~2.3` to `^2.3`, or becomes optional for
  `craft-ui`. Then re-run result 1.
- Any re-run uses the same rule: no `[patch]`, no global egui/winit fork, and no
  downgrade of existing lock entries.

## Reproduce (scratch only)

```bash
# Result 1: add a workspace member `spikes/craft-ui-probe` with
#   craft-ui = { git = "https://github.com/storytold/craft-libs", rev = "b4afa6d…", default-features = false }
cargo check -p craft-ui-probe --target-dir .codex-target -j 2      # resolution error
# Result 2: same Cargo.toml as a standalone `[workspace]` crate (see isolated-probe-Cargo.toml.txt)
cargo check --target-dir target -j 2                               # 5 errors
# Result 3: copy crates/ui/src/tabs/mod.rs into a crate depending only on egui 0.37.0
cargo check --offline                                              # Finished
```

Network was used once, to fetch craft-libs and its crates. Results 1 and 3 used
the existing `egui_test` lockfile unchanged; result 2 created its own lockfile in
scratch.
