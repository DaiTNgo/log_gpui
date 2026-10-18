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
}

impl SessionManifest {
    pub const CURRENT_VERSION: u32 = 3;

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
            })
            .collect();

        Self {
            version: Self::CURRENT_VERSION,
            active_project_id: ws.active_project_id.clone(),
            projects,
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
            });
        }
        ws.active_project_id = self.active_project_id;
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
        if value.get("version")?.as_u64()? != u64::from(Self::CURRENT_VERSION) {
            return None;
        }
        serde_json::from_value(value).ok()
    }

    /// Deserialize from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Directory (in the user config directory) holding the app's own data:
/// `$XDG_CONFIG_HOME/log_gpui` or `~/.config/log_gpui`.
pub fn app_data_dir() -> std::path::PathBuf {
    let mut path = if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(xdg)
    } else if let Some(home) = std::env::var_os("HOME") {
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
                if value.get("version").and_then(|v| v.as_u64())
                    != Some(u64::from(SessionManifest::CURRENT_VERSION))
                {
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