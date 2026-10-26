# Storytold and desktop-host source assessment

Assessment date: 2026-10-26. Source is read-only; no production file was changed.
craft-libs pin: `b9ba47f58902e839a8f102b824bb9f0beaac335c`.
EtabExtension pin: `9eb1c8792a1d41f3f81c60428afe89f1655782ba`.
Lab base: PR #3 head `61f6b74adbbd38255a5736fb0de7f432a3e82cd4`.

## Storytold reuse matrix

The current [workspace manifest](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/Cargo.toml)
targets Rust 1.95 and **egui 0.37**, superseding #5's historical 0.36 observation.
It patches the egui family to `storytold/egui` commit
`6d518fc799681571963842f95b55a6b68431b15a` and winit to
`a517b3846ee8457d84ee4975110bfda6797c44dc`. Dependency workspace patches do not
propagate into this consumer. Matching semver alone is insufficient.

| Source / symbols | Classification | Compatibility and ownership decision |
|---|---|---|
| [`docking/model.rs`: `Layout`, `Action`, `Layout::validate`, `Permissions::PROTECTED`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/ui/src/docking/model.rs) | Direct-reuse candidate | Reusable layout/ID validation and protected viewport policy; not engineering state. Consumer compile probe passes with registry egui 0.37 and serde. Windows behavior remains untested. |
| [`docking/workspace.rs`: `Workspace::validate`, `apply`, `apply_all`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/ui/src/docking/workspace.rs) | Direct-reuse candidate | Workspace persistence and layout changes remain separate from selected project/version. Corrupt/duplicate-ID and recovery tests are needed in #5. |
| [`docking/surface.rs`: `DockArea`, `DockOutput`, `typing_window`, `send_keys`, `chrome_has_focus`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/ui/src/docking/surface.rs) | Compatibility adaptation | Focus routing and drag ownership are useful patterns. A WebView child has its own HWND input/composition behavior; neither library guarantees compatibility. |
| [`docking/render/native.rs`: native input handling](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/ui/src/docking/render/native.rs#L1193) | Compatibility adaptation; blocked on stock API | `Event::ImeResetCompleted` is not present in registry egui 0.37. A consumer probe with `craft-ui/native-windows` fails E0599 at this symbol. No egui fork is adopted here. |
| [`docking/native_tests.rs`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/ui/src/docking/native_tests.rs), [`testing.rs`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/ui/src/docking/testing.rs) | Reference only | Useful focus/IME/floating and headless-driver scenarios. Reading tests is not executing them against this host. |
| [`craft-settings`: `Settings`, `SettingsStore`, `Location`, schema migration/recovery](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/settings/src/lib.rs), [`paths.rs`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/settings/src/paths.rs) | Reference/adapt | Craft chooses env/portable/platform TOML settings and migration/backup policy. Production uses Tauri app-settings.json; no silent directory, format, or schema switch. |
| [`craft-atomic`: `WriteOptions`, `atomic_write`, `write_new`, `RenameRetry`](https://github.com/storytold/craft-libs/blob/b9ba47f58902e839a8f102b824bb9f0beaac335c/crates/atomic/src/lib.rs) | Future direct-reuse candidate | Independent host settings only after policy review. Existing version/config writes remain authoritative ext-api/ext-db operations. |
| Craft icons/visual themes/PDF/CAD components | N/A for Stage A | No asset/theme or unrelated rendering adoption. |

All craft-libs dependencies remain **unadopted** in this PR. A scratch consumer
using path source at the exact pinned checkout, registry `egui = "=0.37.0"`,
and `craft-ui` serde built with `cargo check --offline`. Enabling
`craft-ui/native-windows` failed at `ImeResetCompleted`. This is source/API
compatibility evidence only. It is not native dock/focus/DPI proof.
The sources declare MIT OR Apache-2.0. No Craft implementation is copied.

## Production host responsibility inventory

All source links below use the same EtabExtension pin. Native replacements are
proposals unless implemented in a separately reviewed slice.

| Source / symbol | Current owner | Native replacement / classification | Risk / status |
|---|---|---|---|
| [`state.rs`: `AppState`, `scene_loads`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/state.rs) | Desktop shell | New native service; directly reuse ext-api SceneLoadGuard | Active project/version/calc identity and supersession; #9 first |
| [`state.rs`: `SidecarProcessState::start`, shared client, `shutdown`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/state.rs) | Desktop shell + ext-sidecar | Adapt ownership policy; reuse ext-sidecar transport | Exactly one client; sticky startup refusal must remain observable; #13 mocks first |
| [`sidecar_identity.rs`: `SidecarLock`, required capabilities, package mode](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/sidecar_identity.rs) | Desktop bootstrap | Adapt validation boundary after ADR | SHA/build/protocol/capability/executable identity, truthful override state; no fallback spawn |
| [`commands/common.rs`: `app_context`, `inject_shared_sidecar`, `load_project_state`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/commands/common.rs) | Command adapters | New native typed boundary; reuse ext-api calls | Read-only adapter deliberately uses new_read_only and no sidecar; never reproduce VCS rules |
| [`commands/model.rs`: scene/source worker and load ticket settlement](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/commands/model.rs) | Shell scheduling | Native worker/channel; direct authoritative packet/checked-source reuse | Reject stale project/version/fingerprint before changing selection or citation |
| [`commands/common.rs`: CommandResponse/error mapping](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/commands/common.rs), [`contract.rs`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/contract.rs) | IPC boundary | Native typed result/error states; source chain retained | Preserve typed refusals/guards; do not replace domain rules with message parsing |
| [`lib.rs`: Builder plugins and single-instance`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/lib.rs) | Host | eframe/winit host + a separately evaluated instance policy | Single-instance, dialogs, shell, filesystem and network permissions need explicit equivalents; no new crates yet |
| [`lib.rs`: `install_native_menu`, menu event emission](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/lib.rs) | Host/menu dispatch | Adapt native commands/shortcuts | Focus/IME and WebView shortcuts must not trigger unrelated commands; NOT TESTED |
| [`commands/system.rs`: `open_path`, `open_folder`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/commands/system.rs) | Host OS integration | Typed worker; evaluate minimal OS/open adapter | Dialog/path validation, blocking shell calls and recovery remain host tasks |
| [`commands/common.rs`: `APP_SETTINGS_FILE`, app config store](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/commands/common.rs) | Tauri store | Adapt exact existing data roots/schema and migration policy | Craft settings cannot silently replace production JSON policy |
| [`lib.rs`: log plugin, on_window_event shutdown](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/src/lib.rs) | Host lifecycle | Native logging policy and owned worker/runtime teardown | Clean close/crash/restart, sidecar refusal diagnostics, no leaked process; #8/#13 |
| [`tauri.conf.json`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/tauri.conf.json), [`capabilities/default.json`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/capabilities/default.json) | Host security/bundle | New native packaging and asset access policy | CSP/origin/file scope, application ID, installer, WebView2 prerequisite; NOT TESTED |
| [`tauri.release.conf.json`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/tauri.release.conf.json), [`build.rs`](https://github.com/tadoEng/EtabExtension/blob/9eb1c8792a1d41f3f81c60428afe89f1655782ba/crates/ext-tauri/build.rs) | Release host/toolchain | New release packaging study; reuse sidecar provenance contract | Installed startup, uninstall, upgrade/rollback, signing, bundled binary pin remain unproven. Do not infer updater support from bundle config. |

Native feasibility does not mean importing `ext-tauri`. Keep ext-api as workflow
authority and ext-sidecar as validated transport. Any extracted host-neutral
service requires an ADR first; production source stays unchanged.

## Wry Stage A host decision

Stock eframe 0.37 `CreationContext`/`Frame` implement `HasWindowHandle`.
Wry 0.55.1 `WebViewBuilder::build_as_child`, `set_bounds`, and `set_visible`
accept safe public APIs using raw-window-handle 0.6. No unsafe block or custom
winit host is needed for the first child-panel compile experiment. See
[eframe 0.37 source](https://docs.rs/eframe/0.37.0/src/eframe/epi.rs.html) and
[Wry 0.55.1 API](https://docs.rs/wry/0.55.1/wry/struct.WebViewBuilder.html).

This is **PARTIAL**, not a viable-host verdict. Child HWND composition keeps the
WebView above native painted overlays. Native floating requires the correct
parent viewport handle and reparent/recreation policy. Focus, IME, pointer
capture, tab traversal, monitor moves and OS DPI require Windows observations.
If those fail, report a blocker and compare a small explicit custom-winit host;
do not silently replace eframe or downgrade egui.
