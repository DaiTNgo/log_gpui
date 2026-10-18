//! Application root: `AppView` state + construction.
//!
//! Logic lives in sibling modules (`tabs`, `projects`, `panels`, `search`, `selection`,
//! `view_settings`, `commands`, `keyboard`, `workspace_items`); rendering lives in `view/`.

mod commands;
mod filter;
mod keyboard;
mod persist;
mod raw;
mod panels;
mod projects;
pub mod rename;
mod search;
mod selection;
mod spaces;
mod tabs;
mod view;
mod view_settings;
mod workspace_items;

use crate::column::ColumnLayoutState;
use crate::filter::FilterState;
use crate::filter_index::FilterIndex;
use crate::model::LogEntry;
use crate::search::SearchState;
use crate::space_view::ParsedFile;
use crate::density::Density;
use crate::ui::floating_panel::{
    FloatingPanelState, PanelGesture, PanelKind,
};
use crate::ui::inspector::InspectorTab;
use crate::workspace::state::{ContextSidebarState, LeftSidebarState, WorkspaceState};
use crate::workspace::working_set::ViewMode;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, UniformListScrollHandle,
    Window,
};
use std::collections::{HashMap, HashSet};
use gpui_component::input::{InputEvent, InputState};
use std::sync::Arc;

/// A destructive action waiting for the user's confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteLogSpace { space_id: String },
    DeleteScratchFile { space_id: String, file_id: String },
}

pub struct AppView {
    pub all_entries: Vec<LogEntry>,
    pub visible_indices: Arc<Vec<usize>>,
    pub filter_state: FilterState,
    pub filter_index: FilterIndex,
    pub filter_ui: filter::FilterUi,
    pub search_state: SearchState,
    pub search_input: Entity<InputState>,
    pub scroll_handle: UniformListScrollHandle,

    pub selected_row_pos: Option<usize>,
    pub inspector_tab: InspectorTab,
    pub is_context_open: bool,
    pub is_palette_open: bool,

    pub file_name: String,
    pub file_format: String,
    pub floating_panels: HashMap<PanelKind, FloatingPanelState>,
    pub panel_gesture: Option<PanelGesture>,
    pub density: Density,
    pub column_layout: ColumnLayoutState,
    pub tail_enabled: bool,

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

    /// Raw View or Parsed View of the focused tab.
    pub view_mode: ViewMode,
    pub raw: raw::RawViewState,
    pub quick_settings_open: bool,
    pub new_space: Option<spaces::NewSpaceDialog>,
    pub confirm: Option<ConfirmAction>,
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

        cx.subscribe_in(
            &search_input,
            window,
            |this: &mut Self, input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    this.search_state.query = input.read(cx).value().to_string();
                    if !this.search_state.query.trim().is_empty() {
                        this.search_state.is_open = true;
                    }
                    this.run_search(cx);
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

        let mut this = Self {
            all_entries,
            visible_indices: Arc::new(Vec::new()),
            filter_state,
            filter_index,
            filter_ui,
            search_state: SearchState::new(),
            search_input,
            scroll_handle,
            selected_row_pos: None,
            inspector_tab: InspectorTab::Fields,
            is_context_open: false,
            is_palette_open: false,
            file_name: String::new(),
            file_format: "Text Log".to_string(),
            floating_panels: HashMap::from([
                (PanelKind::Filter, FloatingPanelState::default_for(PanelKind::Filter)),
                (PanelKind::Marker, FloatingPanelState::default_for(PanelKind::Marker)),
            ]),
            panel_gesture: None,
            density: Density::Compact,
            column_layout,
            tail_enabled: false,
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
            view_mode: ViewMode::Parsed,
            raw: raw::RawViewState::new(),
            quick_settings_open: false,
            new_space: None,
            confirm: None,
            notice: None,
            session_save_gen: 0,
            persistence,
            focus_handle,
        };
        this.restore_active_session(cx);
        this
    }
}
