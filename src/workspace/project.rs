use crate::highlight::HighlightRule;
use crate::palette::RecentColors;
use crate::workspace::investigation::InvestigationCase;
pub use crate::workspace::log_space::{
    ImportOutcome, LogFile, LogSpace, LogSpaceKind, RemoveFileError, SpaceNameError, StoredMarker,
    WrongKind, validate_space_name,
};
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

/// A Project is the root unit of persistence and isolation: an investigation
/// space owning Log Spaces, a working set of open tabs, and investigation
/// cases. Projects never share state with each other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Stable unique project identifier.
    pub id: String,
    /// Display name (e.g. "Payment Service").
    pub name: String,
    /// Color accent badge for environment recognition.
    pub color: ProjectColorAccent,
    /// Log Spaces owned by this project.
    pub log_spaces: Vec<LogSpace>,
    /// Open working set (tabs) belonging to this project.
    pub working_set: WorkingSet,
    /// Investigation cases (incidents) gathered within this project.
    pub investigation_cases: Vec<InvestigationCase>,
    /// Highlight Rules shared by every Log Space of the project, in priority
    /// order (the first rule that matches a row paints it).
    #[serde(default)]
    pub highlight_rules: Vec<HighlightRule>,
    /// The colors used most recently for Markers and Highlight Rules.
    #[serde(default)]
    pub recent_colors: RecentColors,
    /// The Columns section: the columns the Focus Pane shows.
    #[serde(default)]
    pub columns: crate::columns::ColumnSettings,
    /// Extraction Rules read from every Log Space, in priority order.
    #[serde(default)]
    pub extraction_rules: Vec<crate::extract::ExtractionRule>,
    /// Log Formats chosen by hand for Log Spaces and Source Files.
    #[serde(default)]
    pub format_choices: crate::log_format::FormatChoices,
}

impl Project {
    pub fn new(id: String, name: String, color: ProjectColorAccent) -> Self {
        Self {
            id,
            name,
            color,
            log_spaces: Vec::new(),
            working_set: WorkingSet::new(),
            investigation_cases: Vec::new(),
            highlight_rules: Vec::new(),
            recent_colors: RecentColors::new(),
            columns: Default::default(),
            extraction_rules: Vec::new(),
            format_choices: Default::default(),
        }
    }

    /// Whether a Log Space of this project already uses `name` (compared
    /// after trimming).
    pub fn has_space_named(&self, name: &str) -> bool {
        let name = name.trim();
        self.log_spaces.iter().any(|s| s.name == name)
    }

    pub fn log_space(&self, space_id: &str) -> Option<&LogSpace> {
        self.log_spaces.iter().find(|s| s.id == space_id)
    }

    pub fn log_space_mut(&mut self, space_id: &str) -> Option<&mut LogSpace> {
        self.log_spaces.iter_mut().find(|s| s.id == space_id)
    }

    /// Find a file of any Log Space of this project.
    pub fn file(&self, space_id: &str, file_id: &str) -> Option<&LogFile> {
        self.log_space(space_id).and_then(|s| s.file(file_id))
    }

    /// Create a Log Space. The name is trimmed; it must be non-blank and not
    /// used by another Log Space of this project.
    pub fn create_log_space(
        &mut self,
        name: &str,
        kind: LogSpaceKind,
    ) -> Result<&mut LogSpace, SpaceNameError> {
        let name = validate_space_name(name, self.log_spaces.iter().map(|s| s.name.as_str()))?;
        let base = format!("{}--{}", self.id, slug_project_id(&name));
        let mut id = base.clone();
        let mut n = 2;
        while self.log_spaces.iter().any(|s| s.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        self.log_spaces.push(LogSpace::new(id, name, kind));
        Ok(self.log_spaces.last_mut().expect("just pushed"))
    }

    /// Rename a Log Space. Keeping its current name is allowed.
    pub fn rename_log_space(&mut self, space_id: &str, name: &str) -> Result<(), SpaceNameError> {
        let name = validate_space_name(
            name,
            self.log_spaces
                .iter()
                .filter(|s| s.id != space_id)
                .map(|s| s.name.as_str()),
        )?;
        if let Some(space) = self.log_space_mut(space_id) {
            space.name = name;
        }
        Ok(())
    }

    /// Remove a Log Space and close every tab that views it. Imported files
    /// on disk are untouched; the caller deletes a Scratch space's folder.
    pub fn remove_log_space(&mut self, space_id: &str) -> Option<LogSpace> {
        let pos = self.log_spaces.iter().position(|s| s.id == space_id)?;
        self.working_set.close_tabs_for_space(space_id);
        Some(self.log_spaces.remove(pos))
    }

    /// Remove a file from a Log Space and close the File View showing it.
    pub fn remove_file(
        &mut self,
        space_id: &str,
        file_id: &str,
    ) -> Result<LogFile, RemoveFileError> {
        let removed = self
            .log_space_mut(space_id)
            .ok_or(RemoveFileError::NotFound)?
            .remove_file(file_id)?;
        self.working_set.close_tabs_for_file(space_id, file_id);
        Ok(removed)
    }

    /// Open a Log Space in the working set, or focus its existing tab.
    pub fn open_log_space(&mut self, space_id: &str, session: TabSessionState) -> Option<usize> {
        self.working_set.open_tab(
            TabTarget::LogSpace {
                space_id: space_id.to_string(),
            },
            session,
        )
    }

    /// Open one file of a Log Space on its own, or focus its existing tab.
    pub fn open_file_view(
        &mut self,
        space_id: &str,
        file_id: &str,
        session: TabSessionState,
    ) -> Option<usize> {
        self.working_set.open_tab(
            TabTarget::FileView {
                space_id: space_id.to_string(),
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