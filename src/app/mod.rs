//! Application root: `AppView` state + construction.
//!
//! Logic lives in sibling modules (`tabs`, `projects`, `panels`, `search`, `selection`,
//! `view_settings`, `commands`, `keyboard`, `workspace_items`); rendering lives in `view/`.

mod commands;
mod keyboard;
mod panels;
mod projects;
pub mod rename;
mod search;
mod selection;
mod tabs;
mod view;
mod view_settings;
mod workspace_items;

use crate::column::{ColumnLayoutState, FieldMaxChars};
use crate::filter::FilterEngine;
use crate::model::LogEntry;
use crate::sample::generate_sample_logs;
use crate::search::SearchState;
use crate::density::Density;
use crate::ui::filter_popover::QuickLogMenu;
use crate::ui::floating_panel::{
    FloatingPanelState, PanelGesture, PanelKind,
};
use crate::ui::inspector::InspectorTab;
use crate::workspace::project::{LogFileDescriptor, ProjectColorAccent};
use crate::workspace::state::{ContextSidebarState, LeftSidebarState, WorkspaceState};
use crate::workspace::working_set::TabSessionState;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, UniformListScrollHandle,
    Window,
};
use std::collections::{HashMap, HashSet};
use gpui_component::input::{EditorState, InputEvent};
use std::sync::Arc;

pub struct AppView {
    pub all_entries: Vec<LogEntry>,
    pub visible_indices: Arc<Vec<usize>>,
    pub filter_engine: FilterEngine,
    pub search_state: SearchState,
    pub search_input: Entity<EditorState>,
    pub scroll_handle: UniformListScrollHandle,

    pub selected_row_pos: Option<usize>,
    pub inspector_tab: InspectorTab,
    pub is_inspector_open: bool,
    pub is_context_open: bool,
    pub is_palette_open: bool,

    pub active_quick_menu: Option<QuickLogMenu>,

    pub file_name: String,
    pub file_format: String,
    pub floating_panels: HashMap<PanelKind, FloatingPanelState>,
    pub panel_gesture: Option<PanelGesture>,
    pub density: Density,
    pub column_layout: ColumnLayoutState,
    pub tail_enabled: bool,
    pub focused_tag: Option<String>,

    // Issue 01 / 02: multi-tab working set inside an isolated project workspace.
    pub workspace_state: WorkspaceState,
    pub entries_cache: HashMap<String, Vec<LogEntry>>,
    pub max_chars_by_file: HashMap<String, FieldMaxChars>,

    // Issue 02: project switcher modal + inline rename state.
    pub project_switcher_open: bool,
    pub project_switcher_index: usize,
    pub project_renaming: Option<String>,
    pub project_rename_buffer: String,

    // Sidebar state
    pub sidebar_state: LeftSidebarState,
    pub context_sidebar: ContextSidebarState,
    pub expanded_projects: HashSet<String>,

    /// Inline rename of a sidebar row (project or log file), if any.
    pub renaming: Option<rename::RenameState>,

    pub focus_handle: FocusHandle,
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let scroll_handle = UniformListScrollHandle::new();
        let mut all_entries = generate_sample_logs();
        let mut column_layout = crate::column::ColumnLayoutState::new();
        column_layout.update_from_entries(&all_entries, Density::Compact);

        let filter_engine = FilterEngine::new();
        let visible = filter_engine.evaluate(&mut all_entries);
        let visible_indices = Arc::new(visible);

let search_input = cx.new(|cx| {
            EditorState::new(window, cx).placeholder("Search ...")
        });
        {
            let schema = crate::query::schema::LogSchema::for_log_fields();
            search_input.update(cx, |state, _cx| {
                state.lsp_mut().completion_provider = Some(std::rc::Rc::new(
                    crate::query::completion_provider::QueryCompletionProvider::new(schema),
                ));
            });
        }

        let mut search_state = SearchState::new();
        search_state.update_matches(&all_entries, &visible_indices);

        // Seed a default project + working set with the demo file open in a tab.
        let mut workspace_state = WorkspaceState::new();
        workspace_state.create_project("Log Workspace", ProjectColorAccent::Blue);
        {
            let proj = workspace_state.active_project_mut().expect("project created");
            proj.register_file(LogFileDescriptor::new("app.log".into(), "app.log".into()));
            let mut session = TabSessionState::new();
            session.filter = filter_engine.clone();
            session.search = search_state.clone();
            session.column_layout = column_layout.clone();
            session.selected_row = Some(0);
            proj.open_file("app.log", session.clone());

            // Seed demo incident investigation case
            let mut demo_case = crate::workspace::investigation::InvestigationCase::new(
                "INC-101".to_string(),
                "API Gateway 502 Upstream Failure".to_string(),
                "P1",
            );
            demo_case.anchor_line(
                "anc-1".to_string(),
                "app.log",
                42,
                1700000000.0,
                crate::model::LogLevel::Error,
                "HTTP 502 Bad Gateway from upstream payment service".to_string(),
                Some("Root cause TCP reset observed".to_string()),
            );
            proj.investigation_cases.push(demo_case);

            // Seed demo saved view
            let mut err_filter = FilterEngine::new();
            err_filter.add_condition(
                crate::filter::ConditionField::Level,
                crate::filter::ConditionOp::Is,
                "ERROR".to_string(),
            );
            let mut err_session = session;
            err_session.filter = err_filter;
            let demo_view = crate::workspace::saved_view::SavedView::capture(
                "view-err".to_string(),
                "Critical Errors (5xx & Fatal)".to_string(),
                &err_session,
            );
            proj.saved_views.push(demo_view);
        }
        let entries_cache = HashMap::from([("app.log".to_string(), all_entries.clone())]);
        let max_chars_by_file = HashMap::from([("app.log".to_string(), column_layout.max_chars)]);

        cx.subscribe(
            &search_input,
            |this: &mut Self, input, _event: &InputEvent, cx| {
                let text = input.read(cx).text().to_string();
                this.search_state.query = text;
                if !this.search_state.query.trim().is_empty() {
                    this.search_state.is_open = true;
                }
                this.run_search(cx);
            },
        )
        .detach();

        Self {
            all_entries,
            visible_indices,
            filter_engine,
            search_state,
            search_input,
            scroll_handle,
            selected_row_pos: Some(0),
            inspector_tab: InspectorTab::Fields,
            is_inspector_open: false,
            is_context_open: false,
            is_palette_open: false,
            active_quick_menu: None,
            file_name: "app.log".to_string(),
            file_format: "Standard Server Log".to_string(),
            floating_panels: HashMap::from([
                (PanelKind::View, FloatingPanelState::default_for(PanelKind::View)),
                (PanelKind::Filter, FloatingPanelState::default_for(PanelKind::Filter)),
                (PanelKind::Marker, FloatingPanelState::default_for(PanelKind::Marker)),
            ]),
            panel_gesture: None,
            density: Density::Compact,
            column_layout,
            tail_enabled: false,
            focused_tag: None,
            workspace_state,
            entries_cache,
            max_chars_by_file,
            project_switcher_open: false,
            project_switcher_index: 0,
            project_renaming: None,
            project_rename_buffer: String::new(),
            sidebar_state: LeftSidebarState::new(),
            context_sidebar: ContextSidebarState::new(),
            expanded_projects: HashSet::new(),
            renaming: None,
            focus_handle,
        }
    }

    pub fn apply_filters_and_refresh(&mut self, cx: &mut Context<Self>) {
        let visible = self.filter_engine.evaluate(&mut self.all_entries);
        self.visible_indices = Arc::new(visible);
        self.search_state
            .update_matches(&self.all_entries, &self.visible_indices);

        if let Some(pos) = self.selected_row_pos {
            if pos >= self.visible_indices.len() {
                self.selected_row_pos = if self.visible_indices.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
        cx.notify();
    }
}
