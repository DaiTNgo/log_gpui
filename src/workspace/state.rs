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

/// Drawer visibility state machine implementing progressive disclosure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarState {
    /// Fully collapsed: neither rail nor drawer visible.
    Hidden,
    /// 40px rail visible, contextual drawer collapsed to 0px.
    Compact,
    /// 40px rail visible + 240px contextual drawer expanded.
    Expanded,
}

/// Modules reachable from the 40px Activity Rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActivityRailTab {
    Files,
    Investigations,
    SavedViews,
    Markers,
    Settings,
}

impl ActivityRailTab {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActivityRailTab::Files => "Files",
            ActivityRailTab::Investigations => "Investigations",
            ActivityRailTab::SavedViews => "Saved Views",
            ActivityRailTab::Markers => "Markers",
            ActivityRailTab::Settings => "Settings",
        }
    }
}

pub const DEFAULT_DRAWER_WIDTH: f32 = 240.0;
pub const MIN_DRAWER_WIDTH: f32 = 180.0;
pub const MAX_DRAWER_WIDTH: f32 = 500.0;

fn default_drawer_width() -> f32 {
    DEFAULT_DRAWER_WIDTH
}

/// Headless drawer/rail state machine. Governs the interaction between the
/// 40px Activity Rail and the resizable Contextual Drawer without GPUI deps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarStateMachine {
    pub state: SidebarState,
    pub active_tab: ActivityRailTab,
    #[serde(default = "default_drawer_width")]
    pub drawer_width: f32,
    #[serde(skip)]
    pub is_resizing: bool,
}

impl Default for SidebarStateMachine {
    fn default() -> Self {
        Self {
            state: SidebarState::Compact,
            active_tab: ActivityRailTab::Files,
            drawer_width: DEFAULT_DRAWER_WIDTH,
            is_resizing: false,
        }
    }
}
impl SidebarStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clicking the active rail icon: if expanded with that tab active,
    /// collapse to Compact Focus Mode.
    pub fn activate_tab(&mut self, tab: ActivityRailTab) {
        if self.state == SidebarState::Expanded && self.active_tab == tab {
            self.state = SidebarState::Compact;
        } else {
            self.active_tab = tab;
            self.state = SidebarState::Expanded;
        }
    }

    /// `⌘B`: toggle drawer visibility. Expands if hidden/compact (keeping the
    /// current rail tab), collapses to Compact if expanded.
    pub fn toggle_drawer(&mut self) {
        self.state = match self.state {
            SidebarState::Expanded => SidebarState::Compact,
            SidebarState::Compact => SidebarState::Expanded,
            SidebarState::Hidden => SidebarState::Expanded,
        };
    }

    /// Legacy cycle through Expanded -> Compact -> Hidden -> Expanded.
    pub fn cycle(&mut self) {
        self.state = match self.state {
            SidebarState::Expanded => SidebarState::Compact,
            SidebarState::Compact => SidebarState::Hidden,
            SidebarState::Hidden => SidebarState::Expanded,
        };
    }

    /// `⌘⇧E`, `⌘⇧I` etc: activate a specific rail tab and expand the drawer.
    pub fn expand_tab(&mut self, tab: ActivityRailTab) {
        self.active_tab = tab;
        self.state = SidebarState::Expanded;
    }

    pub fn is_expanded(&self) -> bool {
        self.state == SidebarState::Expanded
    }

    /// Whether the 40px rail (and any drawer content area) is visible.
    pub fn drawer_visible(&self) -> bool {
        self.state == SidebarState::Expanded
    }

    pub fn rail_visible(&self) -> bool {
        self.state != SidebarState::Hidden
    }

    pub fn set_drawer_width(&mut self, width: f32) {
        self.drawer_width = width.clamp(MIN_DRAWER_WIDTH, MAX_DRAWER_WIDTH);
    }

    pub fn reset_drawer_width(&mut self) {
        self.drawer_width = DEFAULT_DRAWER_WIDTH;
    }
}