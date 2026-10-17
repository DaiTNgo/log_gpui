# Building desktop apps with `gpui-kit`: patterns, pitfalls, log-viewer notes

## Question

How do other developers build Rust desktop apps with the `gpui-kit` crate (facade over GPUI `gpui-pre-*` + `gpui-component` + `gpui-kit-assets`)? Covered: architecture, hot reload, theming, example apps, pitfalls (0.6.x vs 0.7.x), and log-viewer specifics.

Conventions: `CR/` = `~/.cargo/registry/src/*/`. Versions on disk: gpui-kit 0.6.0/0.6.6/0.7.1/0.7.3, gpui-component same, gpui-pre 0.3.3-0.3.9. This repo's lock resolves gpui-kit 0.7.3, gpui-component 0.7.3, gpui-pre 0.3.9 (`Cargo.lock`). Claims that I could not trace to a primary source are marked **UNVERIFIED**.

## TL;DR

- Entry point in 0.7: `gpui_kit::application().with_assets(..).run(|cx| { gpui_kit::init(cx); gpui_kit::open_window(opts, cx, |w, cx| cx.new(|cx| View::new(w, cx))) })`. `open_window` wraps your view in a `gpui_base::Root` for you; never return a `Root` from the closure. (`CR/gpui-kit-0.7.3/src/lib.rs`, https://gpui-kit.com/docs/getting-started)
- 0.7.0 removed the manual overlay plumbing: `Root::render_dialog_layer` / `render_sheet_layer` / `render_notification_layer` are gone; dialogs/sheets/notifications go through `WindowExt` (`window.open_dialog(cx, build)`). `gpui_component::Root` is now a re-export of `gpui_base::Root`. (GitHub releases page; `CR/gpui-component-0.6.6/src/root.rs:185-242` vs `CR/gpui-component-0.7.3/src/lib.rs`)
- There is **no official code hot reload**. The only built-in live reload is for theme JSON files: `ThemeRegistry::watch_dir`. Code hot reload (cargo-watch etc.) is not documented by gpui-kit; treat it as **UNVERIFIED** for GPUI.
- Style by reading `cx.theme()` tokens and using component defaults; edit the global theme with `Theme::update`/`Theme::change` rather than hardcoding colors.
- Two `IconName`s: `gpui_component::IconName` (101 icons, legacy) vs `gpui_kit::assets::IconName` (all 1,830). The full catalog only renders if you register `gpui_kit::assets::AllAssets`; `Icon::new` accepts either.
- Pin versions. `gpui-kit 0.7.3` hard-pins `gpui-pre =0.3.9`; wildcard (`*`) requirements in a manifest invite mismatched GPUI copies on the next `cargo update`.
- For a large log list: `Table`/`DataTable`, `List` and `VirtualList` are all built on virtualization (`uniform_list` / virtual list); GPUI's own `list()` has `FollowMode::Tail` which fits live log tailing. `EditorState` + `Editor` is the code-editor control (Rope-based, tree-sitter optional).
- The only named production app is Longbridge Pro (closed); no third-party gpui-kit app list exists in primary sources. Best "examples" are the repo's `examples/` and `story` gallery.

## 1. Project structure and app architecture

**Crate layering.** `gpui-kit` re-exports GPUI at the root (`use gpui_kit::*;` is GPUI), plus `gpui_kit::platform`, `::base`, `::component`, `::assets`. Features `component` and `assets` are default. (`CR/gpui-kit-0.7.3/README.md`, table "Path / Crate / Feature"; `CR/gpui-kit-0.7.3/Cargo.toml` `[features]`.) Repo: https://github.com/longbridge/gpui-kit (the `repository` field of every crate's Cargo.toml). A hidden `gpui_kit::gpui` alias re-exports the Kit itself so GPUI macro output resolves (`CR/gpui-kit-0.7.3/src/lib.rs`, comment "Public facade decision").

**Entry point (0.7.x).** Canonical, from `CR/gpui-kit-0.7.3/README.md` and `src/lib.rs`:

```rust
gpui_kit::application().run(|cx| {
    gpui_kit::init(cx);                       // before any component use
    gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Hello))
        .expect("failed to open window");
});
```

- `open_window` signature: `(options, &mut App, build: FnOnce(&mut Window,&mut App)->Entity<V>) -> Result<(AnyWindowHandle, Entity<V>)>`; it builds your view, then `cx.new(|cx| base::Root::new(view, window, cx))`. Docs: "Applications own quit/close actions and confirmation flows. Call `init` before opening application windows. The builder returns application content, not another Root. In an async context, call this inside `cx.update`." (`CR/gpui-kit-0.7.3/src/lib.rs`, doc on `open_window`.)
- Getting-started guide says the same: "Return your content view from the closure, not another `Root`" and registers assets via `application().with_assets(assets::Assets)` before opening windows. (https://gpui-kit.com/docs/getting-started, https://gpui-kit.com/docs/assets)
- `gpui_kit::init` initializes the enabled layers; component init (`gpui_component::init`) sets up theme, input, dock, table, menu, tooltip, notification, popover, etc. and registers the window-state plugin on `Root`. (`CR/gpui-component-0.7.3/src/lib.rs` `pub fn init`; `src/root.rs` `init` -> `Root::register_plugin::<WindowState>`.)
- 0.6.x entry point differed: you built `Root` yourself inside `cx.spawn(...cx.open_window(.., |w,cx| cx.new(|cx| Root::new(view,w,cx))))` (`CR/gpui-kit-0.6.6/README.md`, diff vs 0.7.3).

**Root and overlays.** Base `Root` holds the app view plus "presentation plugins" (`RootPlugin`: `prepare`, `style`, `decorate`); component's `WindowState` plugin owns sheet, dialog stack, notification list, tooltip and native-menu overlays. They render above content automatically (`CR/gpui-base-0.7.3/src/root.rs`, doc on `RootPlugin`; `CR/gpui-component-0.7.3/src/root.rs`). Register plugins before creating windows: "Factories are captured when a root is created."

**TitleBar.** `gpui_component::TitleBar::window_options()` returns `WindowOptions` with transparent titlebar, traffic-light position and `app_owns_titlebar_drag: true`; use it as the base of any window that renders a `TitleBar` (`CR/gpui-component-0.7.3/src/title_bar.rs`). `TITLE_BAR_HEIGHT` = 34px, 80px left padding on macOS for traffic lights. `.on_close_window` only applies on Linux. Client-side decorations are handled via `Decorations::Client` and `window_border()` (replaces the old `Root::bordered`).

**Dialogs, sheets, notifications.** In 0.7 use `WindowExt` (`CR/gpui-component-0.7.3/src/window_ext.rs`), e.g. `window.open_dialog(cx, |dialog, window, cx| ...)` (this repo already does so in `src/ui/settings.rs`). Release notes: "root.open_dialog(build, window, cx) becomes window.open_dialog(cx, build)". (https://github.com/longbridge/gpui-kit/releases, v0.7.0)

**Views, state, subscriptions.** The official "tested consumer recipe" (`examples/ai_recipes`) teaches: the owner entity retains state and subscriptions; controls (Checkbox/Switch/RadioGroup) share one `on_change` convention; changes must survive unrelated redraws; it installs Root and renders layers; interaction test types into input, forces a redraw, types again to catch dropped subscriptions. (https://raw.githubusercontent.com/longbridge/gpui-kit/main/examples/ai_recipes/README.md.) Practical meaning: keep `Subscription`s in a struct field, not dropped.

**Delegate pattern for data controls.** Tables use `TableDelegate` + `Entity<TableState<D>>`; lists use `ListDelegate` + `ListState` (`CR/gpui-component-0.7.3/src/table/delegate.rs`, `src/list/delegate.rs`). Delegate refactors were made in 0.5.0 (releases page).

**Actions / keybindings.** `gpui_kit::actions!(namespace, [A, B])` is the facade's own macro (unit actions without needing a `gpui` crate name) (`CR/gpui-kit-0.7.3/src/lib.rs`). Base binds Tab/Shift-Tab and Copy under key context `"Root"` (`CR/gpui-base-0.7.3/src/root.rs` `init`). Apps call `cx.bind_keys(..)` themselves; the facade says apps own quit/close actions. Nothing more prescriptive in primary sources.

**Testing.** `test-support` feature (dev-dependency) + `#[gpui_kit::test]` + `gpui_kit::test::{TestWindowExt, TestAppContextExt, TestSupportExt, ElementSnapshot}`; headless windows, simulated input, focus/layout assertions; `.test_support()` is a no-op in normal builds. (`CR/gpui-kit-0.7.3/TESTING.md`.) Large-list tests check correctness, not render perf (same file).

## 2. Hot reload and dev workflow

- **Themes: official hot reload exists.** `ThemeRegistry::watch_dir(dir, cx, on_load)` loads themes in the background, installs a `notify::RecommendedWatcher` (non-recursive) and calls `on_load`; if the OS watch limit is hit it logs "theme hot reload is disabled". Not available on wasm. (`CR/gpui-component-0.7.3/src/theme/registry.rs:98-125, 186-220`.) Documented usage: https://gpui-kit.com/component/theme, calling `Theme::update(cx, |t| t.apply_config(&theme))` inside the callback. This repo has `docs/design-system.md` and previously `src/theme/*`; it could point watch_dir at a `themes/` folder for live token tweaks.
- **Code hot reload: not documented.** The repo README, getting-started guide and CLAUDE.md contain no hot reload claim (fetched: https://github.com/longbridge/gpui-kit, https://gpui-kit.com/docs/getting-started, raw CLAUDE.md which only imports AGENTS.md). General Rust options (cargo-watch, hot-lib-reloader) are generic tools; whether they work with GPUI windows is **UNVERIFIED**. I did not find any GPUI-specific primary source.
- **Dev tooling that does exist:** `inspector` feature (GPUI inspector; also compiled in `debug_assertions` for component's `inspector` module: `CR/gpui-component-0.7.3/src/lib.rs` `#[cfg(any(feature = "inspector", debug_assertions))]`); `profiler` feature (frame-event instrumentation, off by default); `measure_enable` env-flag logging for table cell measurement (`CR/gpui-component-0.7.3/src/lib.rs`, `gpui_base::measurement_enabled`; env var name not verified).
- Guide notes: `cargo run`; compare backend via `cargo run --features gpui-fast`; "On macOS, rebuild after changing the manifest so text renders" (https://gpui-kit.com/docs/getting-started). The kit already enables `font-kit` on `gpui-pre-platform` (`CR/gpui-kit-0.7.3/Cargo.toml`, `[target...gpui_platform]` features `font-kit, x11, wayland, runtime_shaders`).
- The site and repo ship AI-agent skills: `npx skills add longbridge/gpui-kit` (skills `gpui-kit`, `gpui-kit-design-guides`) (`CR/gpui-kit-0.7.3/README.md`, hosted in GitHub repo `skills/`).

## 3. Theming and styling

- Access: `use gpui_kit::component::ActiveTheme as _; cx.theme().primary / .background / .foreground`. Full resolved token: `cx.theme().tokens.button_primary` (`.color`, `.background` incl. gradients). (https://gpui-kit.com/component/theme; `CR/gpui-component-0.7.3/src/theme/mod.rs`, `impl ActiveTheme for App`.)
- Mode: `Theme::change(ThemeMode::Dark, None, cx)` loads the registered theme for the mode, even if already current; the window arg is ignored ("every window is refreshed either way"). `Theme::sync_system_appearance(window, cx)` follows OS. `Theme::init` starts in Light, so call `change` after `init` (`theme/mod.rs:36-45, 354-410`). So this repo's duplicate `Theme::change(.., Some(window), ..)` inside the window closure is redundant on 0.7.3.
- Edits: `Theme::update(cx, |theme| ...)` is the supported write path; setting `theme.mode` there reloads that mode's colors, so edit colors in a second `update` (doc comment, `theme/mod.rs:~280-290`). `Theme::apply_config` installs a theme file and switches mode in one step.
- Design tokens from Base: `ColorTokens, RadiusTokens, SemanticThemeTokens, ShadowTokens, SpacingTokens, TypographyTokens` re-exported from `gpui_base`; `Theme::{radius_tokens, spacing_tokens, typography_tokens, shadow_tokens, motion_tokens}` (`theme/mod.rs:505-610`). Prefer these over literal px/colors.
- Root applies theme font family/size, background and foreground itself (`set_rem_size`, `.bg(theme.background)`: see 0.6.6 `Root::render`, `CR/gpui-component-0.6.6/src/root.rs:580-612`; in 0.7 delegated to the plugin's `prepare/style`).
- Theme JSON schema: `.theme-schema.json` and 20+ bundled themes in repo `themes/` (https://github.com/longbridge/gpui-kit file listing; theme page).
- Philosophy (README): "Behavior belongs to the foundation. Presentation belongs to the application." Use `gpui-component` for ready defaults ("productive defaults with theming"); drop to `gpui-base` for fully custom visuals. (`CR/gpui-kit-0.7.3/README.md`... see `CR/gpui-component-0.7.3/README.md`.)
- Styling helpers: `StyledExt`, `h_flex()/v_flex()`, `.v_flex()` on div, `Sizable`, `Disableable` (`CR/gpui-component-0.7.3/src/styled.rs`, `lib.rs`).

## 4. Example apps and repos

Primary sources only; no third-party gpui-kit app lists were found.

- Longbridge Pro, a commercial trading desktop app, is stated to be built on it ("from day one") (README). Closed source; not inspectable.
- Repo `examples/` (https://github.com/longbridge/gpui-kit, README "Examples"): `example-dock` (panels, split, tabs), `example-markdown`, `example-html`, `example-editor` (code editor + LSP + syntax highlighting), `hello_world`, `system_monitor` (real-time charts), `window_title`; plus `examples/ai_recipes` (settings recipe, tested) and a `story` gallery crate (`cargo run` at repo root). The crate tarballs do not include these directories; clone the repo.
- Unit/integration tests in the crate are good API usage references: `CR/gpui-kit-0.7.3/tests/` (`dock.rs`, `input/`, `overlays.rs`, `collections.rs` = virtualized Table/Tree, `menu.rs`, `root.rs`), `CR/gpui-kit-assets-0.7.3/examples/{extra_assets,full_assets,selected_assets}.rs`.
- Older repo `github.com/longbridge/gpui-component` is the same codebase pre-rename (page says 17.9k stars, 2,620 commits, "Used by" empty). Status of the rename is not stated; treat as **UNVERIFIED**.

## 5. Pitfalls

**Version / API churn.**
- 0.6.0 (releases page): `InputState` is single-line only; multi-line -> `TextareaState`, code editor -> `EditorState`; stateful `Table` renamed `DataTable` (`Table` is now a declarative element); `row_selector` -> `row_header`; `Divider` -> `Separator`.
- 0.6.2: dock tiles canvas removed.
- 0.7.0: `gpui_kit::open_window` always wraps in `gpui_base::Root`; `gpui_component::Root` re-exports it; manual `render_*_layer` APIs removed; `Root::bordered`/`window_shadow_size` replaced by `window_border()`; dialog/sheet/notification calls move from `Root` to `WindowExt`; text selection moves to `TextSelection::selected_text(window, cx)`; plot primitives moved to `gpui_kit::base::plot`; `DatePicker` change payload changed. Local confirmation: `render_dialog_layer` exists in `CR/gpui-component-0.6.0/src/root.rs:239` and `0.6.6/src/root.rs:242`; absent from 0.7.3 source (grep). 0.6 Root rendered only content + tooltip/menu overlays; apps had to host the dialog layer themselves (`CR/gpui-kit-0.6.6/tests/overlays.rs:20`).
- 0.7.1: charts animate on first paint (`.appear(false)` opts out); single-choice Questionnaire confirms by default. 0.7.2 is **yanked**; use 0.7.3. 0.7.3: `TableDelegate::visible_rows_changed` reports `0..1` for one-row tables and `0..0` for empty; Markdown table widths change.
- 0.7 new modules: `diff`, `toolbar`, `questionnaire`, `speech` (feature), `time_field`, `resizable` as a module (`diff` of `src/` file lists 0.6.6 vs 0.7.3). `decimal` feature now delegates to gpui-base instead of `rust_decimal` in component.
- GPUI snapshot pin: `gpui-kit 0.7.3` -> `gpui-pre =0.3.9`, `gpui-pre-platform =0.3.9` (docs.rs https://docs.rs/crate/gpui-kit/latest; `CR/gpui-kit-0.7.3/Cargo.toml`). A direct `gpui = { package = "gpui-pre", version = "*" }` in this repo's Cargo.toml works only while it resolves to the same 0.3.9; Cargo features are additive and the guide warns "Don't mix Fast types with a direct dependency on upstream GPUI" (getting-started). Prefer removing the direct `gpui`/`gpui-component` deps and importing via `gpui_kit::*` / `gpui_kit::component::*`, which is the documented facade intent.
- `gpui-kit-0.7.3` latest release date 2026-10-12 (docs.rs).

**Icons.** `gpui_kit::component::IconName` = 101 legacy icons (exhaustive matches OK); `gpui_kit::assets::IconName` = 1,830 shared names (1,818 Lucide 1.43.0 + 12 kit icons). `Icon::new()` accepts both. `Assets` embeds only the 101; `AllAssets` embeds all (+~1.02 MiB binary). Blank icons usually mean the source wasn't registered or the name isn't in the selected set; registering a custom source needs a fallback to `Assets` for component icons; `icon_assets!(Extra, [..])` embeds a subset (`CR/gpui-kit-assets-0.7.3/README.md`; https://gpui-kit.com/docs/assets). Using `gpui_kit::assets::IconName::FileText` as `src/ui/inspector.rs` does therefore requires `with_assets(AllAssets)` (main.rs already does), or `icon_assets!` to shrink.

**macOS.**
- Text rendering requires the `font-kit` feature on the platform crate; gpui-kit enables it. Direct GPUI users must enable it and keep `=0.3.9` aligned (getting-started).
- `Root::new` calls `install_window_hit_test_forwarder` on macOS (not in tests) (`CR/gpui-base-0.7.3/src/root.rs`). Custom Root creation outside `open_window` is therefore not advised.
- `TitleBar::window_options()` sets `app_owns_titlebar_drag: true` so AppKit does not double-handle title bar double clicks (`title_bar.rs`).
- `speech` feature pulls objc2 speech/AVFoundation (macOS) and WinRT (Windows) (`CR/gpui-component-0.7.3/Cargo.toml`); keep off unless needed.
- GPUI rendering tests (`--test rendering`) need a Metal-capable runner (`TESTING.md`).
- wasm builds need nightly (getting-started).

**Other gotchas.** `Theme::init` starts Light, so apps must call `Theme::change` after `init`. Controls that need state (inputs, tables) keep it in `Entity<...State>`; dropping the state/subscription silently stops callbacks (recipe README). `Editor`/`Input` must be built with the matching state type since 0.6.0.

## 6. Log viewer / large lists / editor

- **Table.** `DataTable` + `TableState<D: TableDelegate>`; delegate supplies `rows_count`, `columns_count`, `column`, `render_td`, optional sort, `visible_rows_changed`. Internally uses `uniform_list` and a `VirtualListScrollHandle`, supports fixed/resizable/movable columns, sorting, row/col/cell selection; `scroll_to_row`, `refresh` (`CR/gpui-component-0.7.3/src/table/state.rs` imports; `delegate.rs`). README claims "virtual scrolling ... across hundreds of thousands of rows" and a code editor "stable at 200K lines" (`CR/gpui-component-0.7.3/README.md`, `gpui-kit` README) - vendor claims, not independently benchmarked here.
- **VirtualList.** `v_virtual_list`/`h_virtual_list` allow rows of differing size but "must ensure each column width or row height" (caller supplies sizes); inspired by `uniform_list` (`CR/gpui-base-0.7.3/src/virtual_list.rs` header). `VirtualListScrollHandle::{scroll_to_item, scroll_to_bottom}` exist.
- **uniform_list.** GPUI core: measures the first item and lays out the rest linearly; fastest but needs uniform height (`CR/gpui-pre-0.3.9/src/elements/uniform_list.rs` header). This repo's `src/ui/canvas.rs` already uses it.
- **List.** `ListDelegate` + `ListState`: "Every item should have same height"; supports sections and search (`CR/gpui-component-0.7.3/src/list/delegate.rs`), implemented with `v_virtual_list`.
- **Tail following.** GPUI's `list()` with `ListState::set_follow_mode(FollowMode::Tail)`: auto-scrolls when at the bottom and resumes when the user returns to the bottom; `pause_following_tail()` freezes position (`CR/gpui-pre-0.3.9/src/elements/list.rs:111-125, 617-650`). `gpui_component::message_scroller::MessageScrollerState` is a ready example of this (`ListAlignment::Top`, overdraw 400px, `FollowMode::Tail`) (`CR/gpui-component-0.7.3/src/message_scroller.rs`). `list()` supports variable height rows.
- **Editor input.** `EditorState::new(window, cx)` + `Editor::new(&state)`; `.readonly(true)` keeps selection/copy; `.language("..")`; tree-sitter highlighting needs features `tree-sitter` / `tree-sitter-<lang>`; Rope-based (`Rope`, `RopeExt`), search (`Search` action), LSP provider traits, `TextDecoration`/`RangeDecoration` for highlights (`CR/gpui-component-0.7.3/src/input/mod.rs`, `editor.rs`; usage in `CR/gpui-kit-0.7.3/tests/input/editor.rs`). Single-line search box = `Input`/`InputState`; this repo uses `EditorState` for its query box (`src/app/mod.rs`), consistent with 0.6+ split but heavier than necessary (Input suffices for single line; this is judgment, not a source claim).
- **Scroll bars.** `Theme::set_scrollbar_mode`, `Scrollbar`/`ScrollableMask` in `crate::scroll` (`theme/mod.rs`). Base `scrollbar.rs`, `scroll_bounce.rs` exist for platform feel.
- **Diff view** (new in 0.7): `gpui_component::diff` could help compare logs; not evaluated.
- **Perf caveats from sources:** `measure_enable` is a cached startup debug flag because the table queries it per rendered cell (don't read env per cell); `TESTING.md` says large-list tests assert correctness, not frame time. No official perf guide for 1M-row lists was found.

## Recommendations for this log viewer (log_gpui at /Users/dnt/workspace/log_gpui)

1. **Pin versions.** Replace `"*"` in `Cargo.toml` with `gpui-kit = "0.7.3"` (0.7.2 yanked) and drop the direct `gpui-component`/`gpui` deps if imports can go through `gpui_kit::*`/`gpui_kit::component::*`; otherwise pin `gpui = { package = "gpui-pre", version = "=0.3.9" }` to match gpui-kit. Commit `Cargo.lock`.
2. **Entry point.** Switch `src/main.rs` to `gpui_kit::open_window(TitleBar::window_options()-based opts, cx, |w, cx| cx.new(|cx| AppView::new(w, cx)))` (returns `(handle, Entity<AppView>)`); remove the manual `Root::new` and the `cx.spawn` wrapper. Call `Theme::change(ThemeMode::Dark, None, cx)` once after `init` (the in-closure call is redundant).
3. **Overlays.** Keep using `window.open_dialog(cx, ..)`; never call `Root::render_*_layer` (gone).
4. **Icons.** Keep `AllAssets` while using `gpui_kit::assets::IconName`; if binary size matters, switch to `icon_assets!` for the ~dozen icons used. Don't mix `gpui_component::IconName` and `gpui_kit::assets::IconName` imports in one file.
5. **Hot reload.** There is no supported code hot reload; use `cargo watch -x run`-style restarts (generic, UNVERIFIED with GPUI). For styling, move design tokens to a `themes/*.json` and use `ThemeRegistry::watch_dir` for live reload of colors; the debug-build inspector (`debug_assertions`) helps inspect layout.
6. **Lists.** Log rows have uniform height -> keep `uniform_list` (`src/ui/canvas.rs`) or `DataTable` for the table view; use `gpui::list` + `FollowMode::Tail` only if rows must wrap/vary in height or you need live tail with pause-on-scroll. Do not render all rows; derive visible `Range` from the model, keep row data outside render.
7. **State.** Keep `Subscription`s stored on the owner struct (recipe lesson); add `#[gpui_kit::test]` UI tests via the `test-support` dev-feature for query box + table interactions.
8. **Re-verify before upgrading** to a future 0.8: read https://github.com/longbridge/gpui-kit/releases first; 0.6.0 and 0.7.0 each changed Root, Table and Input.
