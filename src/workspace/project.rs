use crate::workspace::investigation::InvestigationCase;
use crate::workspace::saved_view::SavedView;
use crate::workspace::working_set::{TabSessionState, TabTarget, WorkingSet};
use serde::{Deserialize, Serialize};

/// Identifiable color accent badge assigned to a Project for at-a-glance
/// environment recognition across triage contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectColorAccent {
    Blue,
    Green,
    Amber,
    Purple,
    Red,
    Cyan,
}

impl ProjectColorAccent {
    pub const ALL: [ProjectColorAccent; 6] = [
        ProjectColorAccent::Blue,
        ProjectColorAccent::Green,
        ProjectColorAccent::Amber,
        ProjectColorAccent::Purple,
        ProjectColorAccent::Red,
        ProjectColorAccent::Cyan,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectColorAccent::Blue => "Blue",
            ProjectColorAccent::Green => "Green",
            ProjectColorAccent::Amber => "Amber",
            ProjectColorAccent::Purple => "Purple",
            ProjectColorAccent::Red => "Red",
            ProjectColorAccent::Cyan => "Cyan",
        }
    }

    /// HTML/CSS-style hex color used for the badge swatch.
    pub fn hex(&self) -> &'static str {
        match self {
            ProjectColorAccent::Blue => "#3B82F6",
            ProjectColorAccent::Green => "#22C55E",
            ProjectColorAccent::Amber => "#F59E0B",
            ProjectColorAccent::Purple => "#A855F7",
            ProjectColorAccent::Red => "#EF4444",
            ProjectColorAccent::Cyan => "#06B6D4",
        }
    }
}

/// A registered log file belonging to a Project. Registration is purely a
/// registry entry; the underlying file on disk is never modified or deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogFileDescriptor {
    /// Stable unique identifier (typically the path or file name).
    pub file_id: String,
    /// Display name shown in tabs and drawers.
    pub display_name: String,
    /// Absolute path on disk, if known.
    pub path: Option<String>,
    /// Optional subfolder/service grouping for organized drawers.
    pub group: Option<String>,
}

impl LogFileDescriptor {
    pub fn new(file_id: String, display_name: String) -> Self {
        Self {
            file_id,
            display_name,
            path: None,
            group: None,
        }
    }

    /// Build a descriptor from a filesystem path, deriving the file id and
    /// display name from the path components.
    pub fn from_path(path: &str) -> Self {
        let path_obj = std::path::Path::new(path);
        let display_name = path_obj
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        let group = path_obj
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .filter(|p| !p.is_empty() && *p != ".");
        Self {
            file_id: path.to_string(),
            display_name,
            path: Some(path.to_string()),
            group,
        }
    }
}

/// A Project is the root unit of persistence and isolation: an investigation
/// space owning a registry of log files, a working set of open tabs, saved
/// views, and investigation cases. Projects never share state with each other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Stable unique project identifier.
    pub id: String,
    /// Display name (e.g. "Payment Service").
    pub name: String,
    /// Color accent badge for environment recognition.
    pub color: ProjectColorAccent,
    /// Registered log files owned by this project.
    pub files: Vec<LogFileDescriptor>,
    /// Open working set (tabs) belonging to this project.
    pub working_set: WorkingSet,
    /// Saved filter/view presets scoped to this project.
    pub saved_views: Vec<SavedView>,
    /// Investigation cases (incidents) gathered within this project.
    pub investigation_cases: Vec<InvestigationCase>,
}

impl Project {
    pub fn new(id: String, name: String, color: ProjectColorAccent) -> Self {
        Self {
            id,
            name,
            color,
            files: Vec::new(),
            working_set: WorkingSet::new(),
            saved_views: Vec::new(),
            investigation_cases: Vec::new(),
        }
    }

    /// Register a log file into the project. Returns false (no-op) if a file
    /// with the same file_id is already registered.
    pub fn register_file(&mut self, file: LogFileDescriptor) -> bool {
        if self.files.iter().any(|f| f.file_id == file.file_id) {
            return false;
        }
        self.files.push(file);
        true
    }

    /// Remove a registered file from the project registry without touching
    /// the underlying file on disk. Returns the removed descriptor, if any.
    pub fn unregister_file(&mut self, file_id: &str) -> Option<LogFileDescriptor> {
        let pos = self.files.iter().position(|f| f.file_id == file_id)?;
        Some(self.files.remove(pos))
    }

    /// Look up a registered file descriptor by id.
    pub fn file(&self, file_id: &str) -> Option<&LogFileDescriptor> {
        self.files.iter().find(|f| f.file_id == file_id)
    }

    /// Open a file in the working set. If it is already open, focuses the
    /// existing tab instead of creating a duplicate.
    pub fn open_file(&mut self, file_id: &str, session: TabSessionState) -> Option<usize> {
        self.working_set.open_tab(
            TabTarget::LogFile {
                file_id: file_id.to_string(),
            },
            session,
        )
    }

    /// Open an investigation board tab for the given case.
    pub fn open_investigation_board(&mut self, case_id: &str) -> Option<usize> {
        self.working_set.open_tab(
            TabTarget::InvestigationBoard {
                case_id: case_id.to_string(),
            },
            TabSessionState::new(),
        )
    }

    /// Textual identity used by the fuzzy switcher palette.
    pub fn display_label(&self) -> String {
        format!("{} {}", self.color.as_str(), self.name)
    }
}

/// Generates a stable unique project id from a name.
pub fn slug_project_id(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}