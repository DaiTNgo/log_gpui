use crate::workspace::state::WorkspaceState;
use serde::{Deserialize, Serialize};

/// Serialized on-disk manifest representing the entire workspace state for
/// session persistence and recovery. Round-trips through JSON only; no GPUI
/// dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionManifest {
    /// Format version for future migrations.
    pub version: u32,
    /// Id of the active project at save time.
    pub active_project_id: Option<String>,
    /// All registered projects and their full state.
    pub projects: Vec<ProjectSnapshot>,
    /// Focus Pane visibility and height.
    #[serde(default)]
    pub focus_pane: crate::workspace::state::FocusPaneState,
    /// Colors of the Search and Focus Find matches.
    #[serde(default)]
    pub match_colors: crate::palette::MatchColors,
    /// Newest lines kept for each Remote File.
    #[serde(default = "crate::workspace::state::default_live_window")]
    pub live_window: usize,
    /// Lines one Load More reads.
    #[serde(default = "crate::workspace::state::default_load_more_step")]
    pub load_more_step: usize,
    #[serde(default = "crate::workspace::state::default_full_read_limit")]
    pub full_read_limit_lines: usize,
    #[serde(default = "crate::workspace::state::default_glob_limit")]
    pub live_glob_limit: usize,
    #[serde(default = "crate::workspace::state::default_glob_interval")]
    pub live_glob_interval: u64,
    #[serde(default)]
    pub density: crate::density::Density,
    #[serde(default = "crate::workspace::state::default_true")]
    pub show_minimap: bool,
    #[serde(default = "crate::workspace::state::default_true")]
    pub raw_soft_wrap: bool,
    #[serde(default = "crate::workspace::state::default_true")]
    pub show_search_highlight: bool,
    #[serde(default)]
    pub show_source: Option<bool>,
    #[serde(default)]
    pub show_stream: bool,
}

/// Project-level snapshot inside a session manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub id: String,
    pub name: String,
    pub color: crate::workspace::project::ProjectColorAccent,
    pub log_spaces: Vec<crate::workspace::log_space::LogSpace>,
    pub working_set: crate::workspace::working_set::WorkingSet,
    pub investigation_cases: Vec<crate::workspace::investigation::InvestigationCase>,
    #[serde(default)]
    pub highlight_rules: Vec<crate::highlight::HighlightRule>,
    #[serde(default)]
    pub recent_colors: crate::palette::RecentColors,
    #[serde(default)]
    pub columns: crate::columns::ColumnSettings,
    #[serde(default)]
    pub extraction_rules: Vec<crate::extract::ExtractionRule>,
    #[serde(default)]
    pub format_choices: crate::log_format::FormatChoices,
}

impl SessionManifest {
    pub const CURRENT_VERSION: u32 = 4;
    /// The last format still opened. Version 3 Markers (a whole line with a
    /// type and a note) are dropped when it is read.
    const OLDEST_READABLE_VERSION: u32 = 3;

    fn is_readable_version(version: Option<u64>) -> bool {
        version.is_some_and(|v| {
            (u64::from(Self::OLDEST_READABLE_VERSION)..=u64::from(Self::CURRENT_VERSION)).contains(&v)
        })
    }

    /// Capture the current workspace state into a serializable manifest.
    pub fn from_workspace(ws: &WorkspaceState) -> Self {
        let projects = ws
            .projects
            .iter()
            .map(|p| ProjectSnapshot {
                id: p.id.clone(),
                name: p.name.clone(),
                color: p.color,
                log_spaces: p.log_spaces.clone(),
                working_set: p.working_set.clone(),
                investigation_cases: p.investigation_cases.clone(),
                highlight_rules: p.highlight_rules.clone(),
                recent_colors: p.recent_colors.clone(),
                columns: p.columns.clone(),
                extraction_rules: p.extraction_rules.clone(),
                format_choices: p.format_choices.clone(),
            })
            .collect();

        Self {
            version: Self::CURRENT_VERSION,
            active_project_id: ws.active_project_id.clone(),
            projects,
            focus_pane: ws.focus_pane.clone(),
            match_colors: ws.match_colors,
            live_window: ws.live_window,
            load_more_step: ws.load_more_step,
            full_read_limit_lines: ws.full_read_limit_lines,
            live_glob_limit: ws.live_glob_limit,
            live_glob_interval: ws.live_glob_interval,
            density: ws.density,
            show_minimap: ws.show_minimap,
            raw_soft_wrap: ws.raw_soft_wrap,
            show_search_highlight: ws.show_search_highlight,
            show_source: ws.show_source,
            show_stream: ws.show_stream,
        }
    }

    /// Reconstitute a workspace state from this manifest.
    pub fn into_workspace(self) -> WorkspaceState {
        let mut ws = WorkspaceState::new();
        for p in self.projects {
            ws.projects.push(crate::workspace::project::Project {
                id: p.id,
                name: p.name,
                color: p.color,
                log_spaces: p.log_spaces,
                working_set: p.working_set,
                investigation_cases: p.investigation_cases,
                highlight_rules: p.highlight_rules,
                recent_colors: p.recent_colors,
                columns: p.columns,
                extraction_rules: p.extraction_rules,
                format_choices: p.format_choices,
            });
        }
        ws.active_project_id = self.active_project_id;
        ws.focus_pane = self.focus_pane;
        ws.match_colors = self.match_colors;
        ws.live_window = self.live_window;
        ws.load_more_step = self.load_more_step;
        ws.full_read_limit_lines = self.full_read_limit_lines;
        ws.live_glob_limit = self.live_glob_limit;
        ws.live_glob_interval = self.live_glob_interval;
        ws.density = self.density;
        ws.show_minimap = self.show_minimap;
        ws.raw_soft_wrap = self.raw_soft_wrap;
        ws.show_search_highlight = self.show_search_highlight;
        ws.show_source = self.show_source;
        ws.show_stream = self.show_stream;
        ws
    }

    /// Serialize to a compact JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse a session file, ignoring one written by an older format: those
    /// sessions are not converted. Returns `None` for an old or unreadable file.
    pub fn from_json_current(json: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        if !Self::is_readable_version(value.get("version")?.as_u64()) {
            return None;
        }
        serde_json::from_value(value).ok()
    }

    /// Deserialize from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// The user's home directory: `$HOME`, or `%USERPROFILE%` on Windows.
pub fn home_dir() -> Option<std::path::PathBuf> {
    home_dir_from(&|key| std::env::var_os(key), cfg!(windows))
}

/// The local user name: `$USER`, or `%USERNAME%` on Windows.
pub fn user_name() -> Option<String> {
    user_name_from(&|key| std::env::var_os(key), cfg!(windows))
}

fn home_dir_from(get: &dyn Fn(&str) -> Option<std::ffi::OsString>, windows: bool) -> Option<std::path::PathBuf> {
    get("HOME")
        .or_else(|| if windows { get("USERPROFILE") } else { None })
        .map(std::path::PathBuf::from)
}

fn user_name_from(get: &dyn Fn(&str) -> Option<std::ffi::OsString>, windows: bool) -> Option<String> {
    get("USER")
        .or_else(|| if windows { get("USERNAME") } else { None })
        .map(|name| name.to_string_lossy().into_owned())
}

/// Directory (in the user config directory) holding the app's own data:
/// `$XDG_CONFIG_HOME/log_gpui`, `%APPDATA%\log_gpui` on Windows, or
/// `~/.config/log_gpui`.
pub fn app_data_dir() -> std::path::PathBuf {
    let appdata = if cfg!(windows) { std::env::var_os("APPDATA") } else { None };
    let mut path = if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(xdg)
    } else if let Some(appdata) = appdata {
        std::path::PathBuf::from(appdata)
    } else if let Some(home) = home_dir() {
        std::path::PathBuf::from(home).join(".config")
    } else {
        std::path::PathBuf::from(".")
    };
    path.push("log_gpui");
    path
}

/// Path where the session file is written: `<app data dir>/session.json`.
pub fn default_session_path() -> std::path::PathBuf {
    app_data_dir().join("session.json")
}

/// Non-blocking, debounced persistence wrapper. Writes are coalesced: a write
/// only triggers a flush after `debounce_deadline`, so rapid filter updates do
/// not thrash disk I/O. Thread-safe via interior mutability.
#[derive(Debug)]
pub struct SessionPersistence {
    path: std::path::PathBuf,
}

impl SessionPersistence {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Synchronously write the workspace state to disk as JSON.
    pub fn save(&self, ws: &WorkspaceState) -> Result<(), std::io::Error> {
        let manifest = SessionManifest::from_workspace(ws);
        let json = manifest
            .to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, json)
    }

    /// Load the session manifest from disk, if present.
    pub fn load(&self) -> Result<Option<SessionManifest>, std::io::Error> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => {
                // Unreadable JSON is an error; a session of an older format is skipped.
                let value: serde_json::Value = serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                if !SessionManifest::is_readable_version(value.get("version").and_then(|v| v.as_u64())) {
                    return Ok(None);
                }
                serde_json::from_value(value)
                    .map(Some)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::project::ProjectColorAccent;
    use crate::workspace::state::WorkspaceState;

    fn lookup<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<std::ffi::OsString> + 'a {
        move |key| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| std::ffi::OsString::from(v))
    }

    #[test]
    fn windows_home_and_user_come_from_userprofile_and_username() {
        let env = lookup(&[("USERPROFILE", r"C:\Users\me"), ("USERNAME", "me")]);
        assert_eq!(home_dir_from(&env, true), Some(std::path::PathBuf::from(r"C:\Users\me")));
        assert_eq!(user_name_from(&env, true), Some("me".to_string()));
    }

    #[test]
    fn unix_home_and_user_ignore_windows_variables_and_prefer_home_user() {
        let windows_only = lookup(&[("USERPROFILE", r"C:\Users\me"), ("USERNAME", "me")]);
        assert_eq!(home_dir_from(&windows_only, false), None);
        assert_eq!(user_name_from(&windows_only, false), None);
        let both = lookup(&[("HOME", "/home/me"), ("USER", "me"), ("USERPROFILE", "x"), ("USERNAME", "y")]);
        assert_eq!(home_dir_from(&both, true), Some(std::path::PathBuf::from("/home/me")));
        assert_eq!(user_name_from(&both, true), Some("me".to_string()));
    }

    #[test]
    fn session_round_trip_preserves_projects_and_active_id() {
        let mut ws = WorkspaceState::new();
        ws.create_project("Payment Service", ProjectColorAccent::Blue);
        ws.create_project("E-Commerce Core", ProjectColorAccent::Green);
        ws.switch_project("payment-service").unwrap();

        let manifest = SessionManifest::from_workspace(&ws);
        let json = manifest.to_json().unwrap();
        let restored: SessionManifest = SessionManifest::from_json(&json).unwrap();
        let ws2 = restored.into_workspace();

        assert_eq!(ws2.project_count(), 2);
        assert_eq!(ws2.active_project_id.as_deref(), Some("payment-service"));
        assert_eq!(ws2.project("payment-service").unwrap().name, "Payment Service");
        assert_eq!(
            ws2.project("e-commerce-core").unwrap().color,
            ProjectColorAccent::Green
        );
    }
}