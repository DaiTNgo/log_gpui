use crate::workspace::project::{Project, ProjectColorAccent, slug_project_id};
use crate::workspace::working_set::WorkingSet;
use serde::{Deserialize, Serialize};

/// Top-level `WorkspaceState`: the root unit of persistence and isolation
/// managing all registered projects and the currently active project.
///
/// This is the primary headless test seam for the entire log editor
/// workspace domain. All project lifecycle, working set operations, tab
/// switching, investigation anchoring, and session persistence flow through
/// this type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
    /// All registered projects, in registration order.
    pub projects: Vec<Project>,
    /// Id of the currently active project, if any.
    pub active_project_id: Option<String>,
    /// The Focus Pane's visibility and height: remembered by the app, not by
    /// a tab.
    #[serde(default)]
    pub focus_pane: FocusPaneState,
    /// Colors of the Search and Focus Find matches: an app setting.
    #[serde(default)]
    pub match_colors: crate::palette::MatchColors,
    /// Newest lines kept for each Remote File: an app setting.
    #[serde(default = "default_live_window")]
    pub live_window: usize,
    /// Lines one Load More reads: an app setting.
    #[serde(default = "default_load_more_step")]
    pub load_more_step: usize,
    /// Most lines one Open in full reads: an app setting.
    #[serde(default = "default_full_read_limit")]
    pub full_read_limit_lines: usize,
    /// Most files one Live Glob reads at once: an app setting.
    #[serde(default = "default_glob_limit")]
    pub live_glob_limit: usize,
    /// Seconds between two checks of a Live Glob's folder: an app setting.
    #[serde(default = "default_glob_interval")]
    pub live_glob_interval: u64,
    /// Row density of the log table: an app setting.
    #[serde(default)]
    pub density: crate::density::Density,
    /// Whether the minimap is shown: an app setting.
    #[serde(default = "default_true")]
    pub show_minimap: bool,
    /// Whether the Raw View wraps long lines: an app setting.
    #[serde(default = "default_true")]
    pub raw_soft_wrap: bool,
}

/// Default Live Window.
pub fn default_live_window() -> usize {
    100_000
}

/// Default Live Glob file limit.
pub fn default_glob_limit() -> usize {
    50
}

/// Default Live Glob scan interval, in seconds.
pub fn default_glob_interval() -> u64 {
    5
}

/// Default Full Read Limit.
pub fn default_full_read_limit() -> usize {
    2_000_000
}

/// Default Load More Step.
pub fn default_load_more_step() -> usize {
    10_000
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            active_project_id: None,
            focus_pane: FocusPaneState::default(),
            match_colors: Default::default(),
            live_window: default_live_window(),
            load_more_step: default_load_more_step(),
            full_read_limit_lines: default_full_read_limit(),
            live_glob_limit: default_glob_limit(),
            live_glob_interval: default_glob_interval(),
            density: Default::default(),
            show_minimap: true,
            raw_soft_wrap: true,
        }
    }
}

impl WorkspaceState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new named project with the given color accent and register it
    /// in the workspace. The new project becomes active.
    pub fn create_project(&mut self, name: &str, color: ProjectColorAccent) -> &Project {
        let id = slug_project_id(name);
        let project = Project::new(id.clone(), name.to_string(), color);
        self.projects.push(project);
        self.active_project_id = Some(id);
        // Safe: we just pushed the project.
        self.projects.last().unwrap_or_else(|| unreachable!())
    }

    /// Create a project with an explicit id (used by session recovery).
    pub fn restore_project(
        &mut self,
        id: String,
        name: String,
        color: ProjectColorAccent,
        working_set: WorkingSet,
    ) -> &mut Project {
        let project = Project {
            id: id.clone(),
            name,
            color,
            log_spaces: Vec::new(),
            working_set,
            investigation_cases: Vec::new(),
            highlight_rules: Vec::new(),
            recent_colors: Default::default(),
            columns: Default::default(),
            format_choices: Default::default(),
        };
        self.projects.push(project);
        let idx = self.projects.len() - 1;
        &mut self.projects[idx]
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn project_mut(&mut self, id: &str) -> Option<&mut Project> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    pub fn project_index(&self, id: &str) -> Option<usize> {
        self.projects.iter().position(|p| p.id == id)
    }

    /// The currently active project, if any.
    pub fn active_project(&self) -> Option<&Project> {
        self.active_project_id
            .as_deref()
            .and_then(|id| self.project(id))
    }

    pub fn active_project_mut(&mut self) -> Option<&mut Project> {
        self.active_project_id
            .as_deref()
            .map(|id| id.to_string())
            .and_then(|id| self.project_mut(&id))
    }

    pub fn is_active(&self, project_id: &str) -> bool {
        self.active_project_id.as_deref() == Some(project_id)
    }

    /// Switch to a different project, isolating working set/files/canvas.
    pub fn switch_project(&mut self, project_id: &str) -> Option<()> {
        if self.project(project_id).is_none() {
            return None;
        }
        self.active_project_id = Some(project_id.to_string());
        Some(())
    }

    /// Rename an existing project. Returns false if the project is unknown.
    pub fn rename_project(&mut self, project_id: &str, new_name: &str) -> bool {
        match self.project_mut(project_id) {
            Some(project) => {
                project.name = new_name.to_string();
                true
            }
            None => false,
        }
    }

    /// Remove a project from the workspace. Does NOT delete files on disk.
    /// If the active project is removed, the most recently registered
    /// remaining project (if any) becomes active.
    pub fn remove_project(&mut self, project_id: &str) -> Option<Project> {
        let idx = self.project_index(project_id)?;
        let removed = self.projects.remove(idx);

        if self.active_project_id.as_deref() == Some(project_id) {
            self.active_project_id = self.projects.last().map(|p| p.id.clone());
        }

        Some(removed)
    }

    /// Number of registered projects.
    pub fn project_count(&self) -> usize {
        self.projects.len()
    }

    /// Whether any registered project already uses this display name.
    pub fn project_named(&self, name: &str) -> bool {
        self.projects.iter().any(|p| p.name == name)
    }

    /// Whether the switch from `from` to `to` would leak state.
    /// Projects are strictly isolated: no two projects share Log Spaces or tabs.
    pub fn projects_are_isolated(&self, a: &str, b: &str) -> bool {
        match (self.project(a), self.project(b)) {
            (Some(pa), Some(pb)) => {
                let a_spaces: std::collections::HashSet<_> =
                    pa.log_spaces.iter().map(|s| s.id.clone()).collect();
                let b_spaces: std::collections::HashSet<_> =
                    pb.log_spaces.iter().map(|s| s.id.clone()).collect();
                a_spaces.is_disjoint(&b_spaces)
            }
            _ => true,
        }
    }
}

pub const DEFAULT_DRAWER_WIDTH: f32 = 240.0;
pub const MIN_DRAWER_WIDTH: f32 = 180.0;
pub const MAX_DRAWER_WIDTH: f32 = 500.0;
pub const CONTEXT_SIDEBAR_WIDTH: f32 = 320.0;

pub const DEFAULT_FOCUS_PANE_HEIGHT: f32 = 240.0;
pub const MIN_FOCUS_PANE_HEIGHT: f32 = 80.0;
/// The view above the Focus Pane never gets shorter than this.
pub const MIN_VIEW_HEIGHT: f32 = 120.0;

fn default_focus_pane_height() -> f32 {
    DEFAULT_FOCUS_PANE_HEIGHT
}

/// Headless state of the Focus Pane, the full-width pane under the view and
/// both sidebars.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusPaneState {
    /// Expanded. Collapsed, the pane shows only its strip of tabs; it expands
    /// by itself when a Filter or Search Expression appears.
    #[serde(default)]
    pub visible: bool,
    #[serde(default = "default_focus_pane_height")]
    pub height: f32,
    #[serde(skip)]
    pub is_resizing: bool,
}

impl Default for FocusPaneState {
    fn default() -> Self {
        Self { visible: false, height: DEFAULT_FOCUS_PANE_HEIGHT, is_resizing: false }
    }
}

impl FocusPaneState {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Set the height, keeping at least `MIN_FOCUS_PANE_HEIGHT` for the pane
    /// and `MIN_VIEW_HEIGHT` for what is above it within `available` pixels
    /// (the height between the window chrome and the status bar).
    pub fn set_height(&mut self, height: f32, available: f32) {
        self.height = Self::clamp_height(height, available);
    }

    pub fn clamp_height(height: f32, available: f32) -> f32 {
        let max = (available - MIN_VIEW_HEIGHT).max(MIN_FOCUS_PANE_HEIGHT);
        height.clamp(MIN_FOCUS_PANE_HEIGHT, max)
    }
}

fn default_drawer_width() -> f32 {
    DEFAULT_DRAWER_WIDTH
}

pub fn default_true() -> bool {
    true
}

/// Headless state of the left sidebar, which manages Projects (project tree
/// with each project's log files) and hosts Settings at the bottom.
///
/// Toggled from the window header (next to the macOS traffic lights) or `⌘B`.
/// There is no activity rail: the sidebar is either fully visible or hidden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeftSidebarState {
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_drawer_width")]
    pub width: f32,
    #[serde(skip)]
    pub is_resizing: bool,
}

impl Default for LeftSidebarState {
    fn default() -> Self {
        Self {
            visible: true,
            width: DEFAULT_DRAWER_WIDTH,
            is_resizing: false,
        }
    }
}

impl LeftSidebarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// `⌘B` / header toggle: Hidden <-> Expanded.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(MIN_DRAWER_WIDTH, MAX_DRAWER_WIDTH);
    }

    pub fn reset_width(&mut self) {
        self.width = DEFAULT_DRAWER_WIDTH;
    }
}

/// Collapsible sections of the right context sidebar (scoped to the focused
/// log file), stacked top to bottom in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextSection {
    Filter,
    Highlights,
    /// Which columns the Focus Pane shows.
    Columns,
    Inspector,
}

impl ContextSection {
    pub const ALL: [ContextSection; 4] = [
        ContextSection::Filter,
        ContextSection::Highlights,
        ContextSection::Columns,
        ContextSection::Inspector,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ContextSection::Filter => "Filter",
            ContextSection::Highlights => "Highlights",
            ContextSection::Columns => "Columns",
            ContextSection::Inspector => "Inspector",
        }
    }

    /// Slot in the saved `collapsed` flags (Columns has a flag of its own,
    /// added after them).
    fn index(self) -> Option<usize> {
        match self {
            ContextSection::Filter => Some(0),
            ContextSection::Highlights => Some(1),
            ContextSection::Inspector => Some(2),
            ContextSection::Columns => None,
        }
    }
}

/// Headless state of the right context sidebar: visibility plus which of the
/// Filter / Highlights / Inspector sections are collapsed. It is only
/// meaningful while a log file tab is focused.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSidebarState {
    pub visible: bool,
    collapsed: [bool; 3],
    #[serde(default = "default_true")]
    columns_collapsed: bool,
}

impl Default for ContextSidebarState {
    fn default() -> Self {
        Self {
            visible: false,
            // Highlights starts collapsed so Filter and Inspector get the room.
            collapsed: [false, true, false],
            columns_collapsed: true,
        }
    }
}

impl ContextSidebarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Header toggle: show/hide the sidebar, keeping the section states.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn is_collapsed(&self, section: ContextSection) -> bool {
        match section.index() {
            Some(i) => self.collapsed[i],
            None => self.columns_collapsed,
        }
    }

    fn collapsed_mut(&mut self, section: ContextSection) -> &mut bool {
        match section.index() {
            Some(i) => &mut self.collapsed[i],
            None => &mut self.columns_collapsed,
        }
    }

    pub fn toggle_section(&mut self, section: ContextSection) {
        let c = self.collapsed_mut(section);
        *c = !*c;
    }

    /// Show the sidebar with a specific section expanded.
    pub fn reveal(&mut self, section: ContextSection) {
        *self.collapsed_mut(section) = false;
        self.visible = true;
    }

    /// Whether the sidebar is actually rendered: it needs both the visible
    /// flag and a focused log file tab.
    pub fn is_shown(&self, has_log_file_tab: bool) -> bool {
        self.visible && has_log_file_tab
    }
}
