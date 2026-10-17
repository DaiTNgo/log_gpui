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
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            active_project_id: None,
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
            files: Vec::new(),
            working_set,
            saved_views: Vec::new(),
            investigation_cases: Vec::new(),
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
    /// Projects are strictly isolated: no two projects share files or tabs.
    pub fn projects_are_isolated(&self, a: &str, b: &str) -> bool {
        match (self.project(a), self.project(b)) {
            (Some(pa), Some(pb)) => {
                let a_files: std::collections::HashSet<_> =
                    pa.files.iter().map(|f| f.file_id.clone()).collect();
                let b_files: std::collections::HashSet<_> =
                    pb.files.iter().map(|f| f.file_id.clone()).collect();
                a_files.is_disjoint(&b_files)
            }
            _ => true,
        }
    }
}

pub const DEFAULT_DRAWER_WIDTH: f32 = 240.0;
pub const MIN_DRAWER_WIDTH: f32 = 180.0;
pub const MAX_DRAWER_WIDTH: f32 = 500.0;
pub const CONTEXT_SIDEBAR_WIDTH: f32 = 260.0;

fn default_drawer_width() -> f32 {
    DEFAULT_DRAWER_WIDTH
}

fn default_true() -> bool {
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

/// Sections of the right context sidebar (scoped to the active project and
/// the currently focused log file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextTab {
    Files,
    Investigations,
    SavedViews,
    Markers,
}

impl ContextTab {
    pub const ALL: [ContextTab; 4] = [
        ContextTab::Files,
        ContextTab::Investigations,
        ContextTab::SavedViews,
        ContextTab::Markers,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ContextTab::Files => "Files",
            ContextTab::Investigations => "Investigations",
            ContextTab::SavedViews => "Saved Views",
            ContextTab::Markers => "Markers",
        }
    }
}

/// Headless state of the right context sidebar (Files / Investigations /
/// Saved Views / Markers). Independent from the Inspector panel. It is only
/// meaningful while a log file tab is focused.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSidebarState {
    pub visible: bool,
    pub active_tab: ContextTab,
}

impl Default for ContextSidebarState {
    fn default() -> Self {
        Self {
            visible: false,
            active_tab: ContextTab::Files,
        }
    }
}

impl ContextSidebarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Header toggle: show/hide the sidebar, keeping the current section.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Show the sidebar on a specific section.
    pub fn show_tab(&mut self, tab: ContextTab) {
        self.active_tab = tab;
        self.visible = true;
    }

    /// Whether the sidebar is actually rendered: it needs both the visible
    /// flag and a focused log file tab.
    pub fn is_shown(&self, has_log_file_tab: bool) -> bool {
        self.visible && has_log_file_tab
    }
}
