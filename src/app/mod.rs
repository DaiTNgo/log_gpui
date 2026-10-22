//! Application root: `AppView` state + construction.
//!
//! Logic lives in sibling modules (`tabs`, `projects`, `panels`, `search`, `selection`,
//! `view_settings`, `commands`, `keyboard`, `workspace_items`); rendering lives in `view/`.

pub mod color;
mod connection_details;
mod commands;
pub mod columns;
pub mod line_config;
pub mod formats;
pub mod format_editor;
mod filter;
mod focus;
mod focus_window;
pub mod full_read;
pub mod globs;
mod highlights;
mod keyboard;
mod shortcuts;
pub use shortcuts::shortcut_label;
pub mod live;
pub mod live_more;
pub mod live_trim;
pub mod open_in_full;
mod marking;
mod persist;
pub mod remote;
pub mod remote_command;
pub mod remote_picker;
pub mod quick_connect;
pub mod server_dialog;
pub mod ssh_import;
pub mod raw;
mod panels;
mod projects;
pub mod rename;
mod search;
mod selection;
mod spaces;
pub mod text_select;
mod tabs;
mod view;
mod view_settings;
pub use view_settings::MatchKind;
mod workspace_items;

use crate::column::ColumnLayoutState;
use crate::filter::FilterState;
use crate::filter_index::FilterIndex;
use crate::model::LogEntry;
use crate::focus::{FocusRow, FocusTabs};
use crate::space_view::ParsedFile;
use crate::density::Density;
use crate::ui::floating_panel::{
    FloatingPanelState, PanelGesture, PanelKind,
};
use crate::workspace::state::{ContextSidebarState, LeftSidebarState, WorkspaceState};
use crate::workspace::working_set::ViewMode;
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, Pixels, Point,
    UniformListScrollHandle, Window, point,
};
use gpui_component::color_picker::{ColorPickerEvent, ColorPickerState};
use std::cell::Cell;
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use gpui_component::input::{InputEvent, InputState};
use std::sync::Arc;

/// `n` with a comma between each group of three digits: 12345 -> "12,345".
pub fn format_count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A destructive action waiting for the user's confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteLogSpace { space_id: String },
    DeleteScratchFile { space_id: String, file_id: String },
    /// Clear Remote Files whose lines carry `markers` Markers.
    ClearFiles { file_ids: Vec<String>, markers: usize },
    DeleteServer { server_id: String },
    RemoveProject { project_id: String },
    /// Delete a Log Format a Log Space or Source File has chosen.
    DeleteLogFormat { format_id: String },
}

/// How the open view was refreshed, counted since the app started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefreshCounts {
    /// Every row read again (`apply_filters_and_refresh`).
    pub full: usize,
    /// Only Filter and the Focus Pane (`apply_filter_change`).
    pub filter: usize,
    /// Rows appended after the last one (`apply_rows_appended`).
    pub appended: usize,
    /// The rows built again from every file (`rebuild_open_view_without`).
    pub rebuilds: usize,
    /// Search scans started on a worker thread.
    pub scans: usize,
}

pub struct AppView {
    pub all_entries: Vec<LogEntry>,
    pub visible_indices: Arc<Vec<usize>>,
    pub filter_state: FilterState,
    pub filter_index: FilterIndex,
    pub filter_ui: filter::FilterUi,
    /// The Filter result: the entries that pass it, ascending. The view shows
    /// every entry; Filter only narrows the Focus Pane.
    pub filter_pass: Arc<Vec<usize>>,
    /// The row with the longest message, which sets the view's width. Found
    /// when the rows change, never while drawing.
    pub longest_row: usize,
    /// The Focus Tabs of the open view; the open tab holds the Search.
    pub focus: FocusTabs,
    /// The lines the open Focus Tab lists.
    pub focus_rows: Arc<Vec<FocusRow>>,
    /// The row of `focus_rows` with the longest text (sets the pane's width).
    pub focus_longest: usize,
    pub focus_scroll: UniformListScrollHandle,
    /// The Focus Window, while the Focus Pane lives there. Never saved.
    pub focus_window: Option<focus_window::FocusWindowHandle>,
    pub focus_find_input: Entity<InputState>,
    /// Focus Find's field must be rewritten from the open tab.
    pub focus_find_dirty: bool,
    /// Text selected with the mouse in the Focus Pane (rows are pane rows).
    pub pane_selection: Option<crate::text_selection::TextSelection>,
    pub pane_selecting: bool,
    /// Whether a Filter or Search Expression was in effect at the last render.
    pub pane_has_content: bool,
    /// Where the pane's rows are on screen, written while painting.
    pub pane_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub search_input: Entity<InputState>,
    /// The page of the Settings dialog that is open, and its search field.
    pub settings_section: crate::ui::settings::SettingsSection,
    pub settings_search: Entity<InputState>,
    /// Which Shortcut runs which Command, and where it is saved.
    pub keymap: crate::keymap::Keymap,
    pub keymap_path: std::path::PathBuf,
    /// The Command the Keymap settings wait for a key press for.
    pub keymap_recording: Option<crate::keymap::Command>,
    /// What the last change in the Keymap settings did, or why it did not.
    pub keymap_message: Option<String>,
    /// Holds the key presses while a Shortcut is being recorded.
    pub keymap_record_focus: FocusHandle,
    /// The search field must be rewritten from the tab's query (set when a
    /// tab's session is restored); done at render time, where a window exists.
    pub search_input_dirty: bool,
    pub scroll_handle: UniformListScrollHandle,

    pub selected_row_pos: Option<usize>,
    /// Text selected with the mouse in the Parsed View.
    pub text_selection: Option<crate::text_selection::TextSelection>,
    /// The mouse button is down and the selection follows the pointer.
    pub selecting: bool,
    /// Which way a drag past an edge scrolls: (horizontal, vertical).
    pub autoscroll_dir: (i32, i32),
    pub autoscroll_pointer: Point<Pixels>,
    pub autoscroll_running: bool,
    /// Where the rows are on screen, written while painting and read to turn
    /// a pointer position into a row and a character.
    pub canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// The Highlight Rule paint of each entry of the open view.
    pub highlight_paints: Arc<crate::highlight::RowPaints>,
    /// How many shown rows each Highlight Rule matches, by rule id.
    pub rule_counts: HashMap<u64, usize>,
    /// The floating bar of colors (or the palette of a rule).
    pub color_bar: Option<color::ColorBar>,
    pub color_picker: Entity<ColorPickerState>,
    /// What the last right-click landed on, for the context menu.
    pub cell_target: Option<text_select::CellTarget>,
    /// The "+ Add rule" form of the Highlights section.
    pub rule_form: Option<highlights::RuleForm>,
    pub rule_input: Entity<InputState>,
    pub is_palette_open: bool,

    pub file_name: String,
    pub file_format: String,
    pub floating_panels: HashMap<PanelKind, FloatingPanelState>,
    pub panel_gesture: Option<PanelGesture>,
    pub density: Density,
    /// Whether the minimap shows beside the log table.
    pub show_minimap: bool,
    pub column_layout: ColumnLayoutState,
    /// Live reading: sources, Follow and Pause.
    pub live: live::LiveRuntime,
    /// The newest Search scan asked for: an older one that finishes is
    /// dropped.
    pub search_generation: u64,
    /// The texts of the rows the Search scan reads, kept between scans.
    pub search_lines: crate::search_lines::SearchLines,
    /// Set to stop the Search scan of the generation before it.
    pub search_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Bumped whenever rows left or moved in the open view (not when rows
    /// were only added at the end): a scan of the rows before that is stale.
    pub rows_epoch: u64,
    /// The newest Time Range edit waiting for its debounce.
    pub time_range_generation: u64,
    /// How often the view was refreshed each way (tests and the perf probe
    /// read it to see which path a change took).
    pub refreshes: RefreshCounts,
    /// How often finding the row of a line walked every row (a stale
    /// selected position), and the row that walk found.
    pub entry_scans: std::cell::Cell<usize>,
    entry_found: std::cell::Cell<Option<usize>>,
    /// Servers, the SSH hub and their dialogs.
    pub remote: remote::RemoteUi,
    pub server_dialog: Option<server_dialog::ServerDialog>,

    // Multi-tab working set inside an isolated project workspace.
    pub workspace_state: WorkspaceState,
    /// Every opened file, parsed once and shared by Log Space and File View tabs.
    pub parsed_files: HashMap<String, ParsedFile>,
    /// Files with no timestamp at all in the open Log Space.
    pub no_timestamp_files: HashSet<String>,

    // Project switcher modal + inline rename state.
    pub project_switcher_open: bool,
    pub project_switcher_index: usize,
    pub project_renaming: Option<String>,
    pub project_rename_buffer: String,

    // Sidebar state
    pub sidebar_state: LeftSidebarState,
    pub context_sidebar: ContextSidebarState,
    pub expanded_projects: HashSet<String>,
    /// Log Spaces whose file list is open in the sidebar.
    pub expanded_spaces: HashSet<String>,

    /// Inline rename of a sidebar row (project, Log Space or file), if any.
    pub renaming: Option<rename::RenameState>,
    /// The Log Format editor, when open.
    pub format_editor: Option<format_editor::FormatEditor>,
    /// The Log Format library (app config) and its compiled templates.
    pub formats: formats::FormatLibrary,
    /// The compiled line reading (Detected Fields) and its background reading.
    pub line_cache: line_config::LineCache,
    /// Search of the Detected Fields in the Columns section.
    pub columns_query: Entity<InputState>,

    /// Raw View or Parsed View of the focused tab.
    pub view_mode: ViewMode,
    pub raw: raw::RawViewState,
    pub new_space: Option<spaces::NewSpaceDialog>,
    pub confirm: Option<ConfirmAction>,
    /// Load more with a number the user types.
    pub custom_load: Option<live_more::CustomLoad>,
    /// A light message shown above the status bar until dismissed.
    pub notice: Option<String>,
    pub session_save_gen: u64,
    /// Where the session is saved; `None` keeps the app from touching disk.
    pub persistence: Option<crate::workspace::SessionPersistence>,

    pub focus_handle: FocusHandle,
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_persistence(
            Some(crate::workspace::SessionPersistence::new(
                crate::workspace::session::default_session_path(),
            )),
            window,
            cx,
        )
    }

    pub fn with_persistence(
        persistence: Option<crate::workspace::SessionPersistence>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::with_services(persistence, remote::RemoteUi::load(), window, cx)
    }

    /// Like `with_persistence`, with the Servers, known hosts and secret
    /// store given. Tests use it to keep the app off the user's config and
    /// the OS keychain (see `remote::RemoteUi::with_paths`).
    pub fn with_services(
        persistence: Option<crate::workspace::SessionPersistence>,
        remote: remote::RemoteUi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Column widths restored below are fitted with the measured font.
        crate::ui::mono::measure(cx);
        let focus_handle = cx.focus_handle();
        let scroll_handle = UniformListScrollHandle::new();
        let all_entries: Vec<LogEntry> = Vec::new();
        let column_layout = ColumnLayoutState::new();

        let filter_state = FilterState::new();
        let filter_index = FilterIndex::build(&all_entries);
        let filter_ui = filter::FilterUi::new(window, cx);

        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search lines…  e.g. timeout && (db || cache)")
        });

        // Restore the last session; a session from an older format is ignored.
        // The Log Format library sits beside the session (in the app's
        // config directory).
        let formats_path = persistence
            .as_ref()
            .and_then(|p| p.path().parent().map(|d| d.join("log_formats.json")))
            .unwrap_or_else(crate::log_format::LogFormatStore::default_path);
        let keymap_path = persistence
            .as_ref()
            .and_then(|p| p.path().parent().map(|d| d.join("keymap.json")))
            .unwrap_or_else(crate::keymap::Keymap::default_path);
        let (keymap, keymap_error) = match crate::keymap::Keymap::load(&keymap_path) {
            Ok(keymap) => (keymap, None),
            Err(e) => (Default::default(), Some(format!("keymap.json could not be read ({e}); using the default shortcuts."))),
        };
        let mut workspace_state = persistence
            .as_ref()
            .and_then(|p| p.load().ok().flatten())
            .map(|manifest| manifest.into_workspace())
            .unwrap_or_default();
        if workspace_state.projects.is_empty() {
            workspace_state = WorkspaceState::new();
            workspace_state.create_project("Log Workspace", crate::workspace::ProjectColorAccent::Blue);
        } else if workspace_state.active_project().is_none() {
            let first = workspace_state.projects[0].id.clone();
            workspace_state.active_project_id = Some(first);
        }
        let expanded_spaces: HashSet<String> = workspace_state
            .projects
            .iter()
            .flat_map(|p| p.log_spaces.iter().map(|s| s.id.clone()))
            .collect();
        let expanded_projects: HashSet<String> = workspace_state
            .active_project_id
            .iter()
            .cloned()
            .collect();

        let settings_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings…"));
        cx.subscribe_in(
            &settings_search,
            window,
            |_this: &mut Self, _input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    // The open dialog reads the field when it draws.
                    window.refresh();
                    cx.notify();
                }
            },
        )
        .detach();

        let focus_find_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Find in this tab…")
        });
        cx.subscribe_in(
            &focus_find_input,
            window,
            |this: &mut Self, input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let text = input.read(cx).value().to_string();
                    if this.focus.find().query != text {
                        this.on_focus_find_changed(&text, cx);
                    }
                }
                InputEvent::PressEnter { shift, .. } => {
                    if *shift {
                        this.focus_find_prev(cx);
                    } else {
                        this.focus_find_next(cx);
                    }
                }
                _ => {}
            },
        )
        .detach();

        cx.subscribe_in(
            &search_input,
            window,
            |this: &mut Self, input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let text = input.read(cx).value().to_string();
                    if this.focus.search().query == text {
                        return;
                    }
                    let search = this.active_search_mut();
                    search.query = text;
                    if !search.query.trim().is_empty() {
                        search.is_open = true;
                    }
                    this.search_text_changed(cx);
                }
                InputEvent::PressEnter { shift, .. } => {
                    if *shift {
                        this.jump_prev_match(cx);
                    } else {
                        this.jump_next_match(cx);
                    }
                }
                _ => {}
            },
        )
        .detach();

        let color_picker = cx.new(|cx| ColorPickerState::new(window, cx));
        cx.subscribe_in(
            &color_picker,
            window,
            |this: &mut Self, _, event: &ColorPickerEvent, _window, cx| {
                if let ColorPickerEvent::Change(Some(color)) = event {
                    this.apply_bar_color(color::color_from_hsla(*color), cx);
                }
            },
        )
        .detach();
        let columns_query = cx.new(|cx| InputState::new(window, cx).placeholder("Find a field…"));
        cx.subscribe(&columns_query, |_this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        let rule_input = cx.new(|cx| InputState::new(window, cx).placeholder("Add value…  (contains)"));
        cx.subscribe_in(
            &rule_input,
            window,
            |this: &mut Self, input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().to_string();
                    if this.rule_form_add_custom(&text, cx) {
                        input.update(cx, |state, cx| {
                            state.set_value(gpui::SharedString::from(""), window, cx)
                        });
                    }
                }
            },
        )
        .detach();

        let mut this = Self {
            all_entries,
            visible_indices: Arc::new(Vec::new()),
            filter_state,
            filter_index,
            filter_ui,
            filter_pass: Arc::new(Vec::new()),
            longest_row: 0,
            focus: FocusTabs::new(),
            focus_rows: Arc::new(Vec::new()),
            focus_longest: 0,
            focus_scroll: UniformListScrollHandle::new(),
            focus_window: None,
            focus_find_input,
            focus_find_dirty: true,
            pane_selection: None,
            pane_selecting: false,
            pane_has_content: false,
            pane_bounds: Rc::new(Cell::new(Bounds::default())),
            search_input,
            settings_section: Default::default(),
            settings_search,
            keymap,
            keymap_path,
            keymap_recording: None,
            keymap_message: None,
            keymap_record_focus: cx.focus_handle(),
            search_input_dirty: true,
            scroll_handle,
            selected_row_pos: None,
            text_selection: None,
            selecting: false,
            autoscroll_dir: (0, 0),
            autoscroll_pointer: point(gpui::px(0.0), gpui::px(0.0)),
            autoscroll_running: false,
            canvas_bounds: Rc::new(Cell::new(Bounds::default())),
            highlight_paints: Arc::new(Vec::new()),
            rule_counts: HashMap::new(),
            color_bar: None,
            color_picker,
            cell_target: None,
            rule_form: None,
            rule_input,
            is_palette_open: false,
            file_name: String::new(),
            file_format: "Text Log".to_string(),
            floating_panels: HashMap::from([
                (PanelKind::Filter, FloatingPanelState::default_for(PanelKind::Filter)),
            ]),
            panel_gesture: None,
            density: Density::Compact,
            show_minimap: true,
            column_layout,
            live: live::LiveRuntime::new(),
            refreshes: RefreshCounts::default(),
            entry_scans: std::cell::Cell::new(0),
            entry_found: std::cell::Cell::new(None),
            search_generation: 0,
            search_lines: Default::default(),
            search_cancel: Default::default(),
            rows_epoch: 0,
            time_range_generation: 0,
            remote,
            server_dialog: None,
            workspace_state,
            parsed_files: HashMap::new(),
            no_timestamp_files: HashSet::new(),
            project_switcher_open: false,
            project_switcher_index: 0,
            project_renaming: None,
            project_rename_buffer: String::new(),
            sidebar_state: LeftSidebarState::new(),
            context_sidebar: ContextSidebarState::new(),
            expanded_projects,
            expanded_spaces,
            renaming: None,
            line_cache: Default::default(),
            format_editor: None,
            formats: formats::FormatLibrary::load(formats_path),
            columns_query,
            view_mode: ViewMode::Parsed,
            raw: raw::RawViewState::new(),
            new_space: None,
            confirm: None,
            custom_load: None,
            notice: keymap_error,
            session_save_gen: 0,
            persistence,
            focus_handle,
        };
        // Closing the main window closes the Focus Window with it.
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_window, cx| {
            weak.update(cx, |this, cx| this.close_focus_window(cx)).ok();
            true
        });
        this.live.window = this.workspace_state.live_window.max(1_000);
        this.live.load_more_step = this.workspace_state.load_more_step.clamp(1_000, 1_000_000);
        this.live.full_read_limit = this.workspace_state.full_read_limit_lines.clamp(10_000, 20_000_000);
        this.live.glob_limit = this.workspace_state.live_glob_limit.clamp(1, 500);
        this.live.glob_interval = this.workspace_state.live_glob_interval.clamp(2, 300);
        this.density = this.workspace_state.density;
        this.column_layout.update_density(this.density);
        this.show_minimap = this.workspace_state.show_minimap;
        this.raw.soft_wrap = this.workspace_state.raw_soft_wrap;
        this.restore_active_session(cx);
        // Remote Copies no Log Space holds any more (the app closed in the
        // middle of removing one) are cleared away.
        crate::remote::copy::sweep(&this.copies_root(), &this.copy_dirs_in_use());
        this.start_live_pump(cx);
        this
    }
}
