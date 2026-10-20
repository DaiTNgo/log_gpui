//! What the file picker remembers across runs, for the whole app: the sort
//! order, and for each Server the last folder opened and the Recent Paths.
//! Kept in `picker_state.json` next to `servers.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// How many Recent Paths a Server keeps.
pub const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortKey {
    #[default]
    Name,
    Modified,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SortOrder {
    pub key: SortKey,
    pub descending: bool,
}

impl SortOrder {
    /// Click a column header: another column sorts ascending by it, the same
    /// column flips the direction.
    pub fn toggled(self, key: SortKey) -> Self {
        if self.key == key {
            Self { key, descending: !self.descending }
        } else {
            Self { key, descending: false }
        }
    }
}

/// A path or Live Glob added from a Server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentPath {
    pub path: String,
    #[serde(default)]
    pub is_glob: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerPicker {
    #[serde(default)]
    pub last_dir: Option<String>,
    #[serde(default)]
    pub recent: Vec<RecentPath>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickerState {
    #[serde(default)]
    pub sort: SortOrder,
    #[serde(default)]
    pub servers: HashMap<String, ServerPicker>,
    /// Server Groups folded in the Server lists (`""` is "Ungrouped").
    #[serde(default)]
    pub collapsed_groups: Vec<String>,
}

impl PickerState {
    pub fn is_group_collapsed(&self, group: &str) -> bool {
        self.collapsed_groups.iter().any(|g| g == group)
    }

    /// Fold or unfold a Server Group in the Server lists.
    pub fn toggle_group(&mut self, group: &str) {
        if let Some(pos) = self.collapsed_groups.iter().position(|g| g == group) {
            self.collapsed_groups.remove(pos);
        } else {
            self.collapsed_groups.push(group.to_string());
        }
    }

    /// A missing or unreadable file is an empty state.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Written to a temporary file first, then renamed over the real one.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    pub fn last_dir(&self, server_id: &str) -> Option<&str> {
        self.servers.get(server_id).and_then(|s| s.last_dir.as_deref())
    }

    pub fn set_last_dir(&mut self, server_id: &str, dir: &str) {
        self.servers.entry(server_id.to_string()).or_default().last_dir = Some(dir.to_string());
    }

    pub fn recent(&self, server_id: &str) -> &[RecentPath] {
        self.servers.get(server_id).map(|s| s.recent.as_slice()).unwrap_or(&[])
    }

    /// Put a path first; a repeat moves up instead of doubling, and only the
    /// newest `MAX_RECENT` stay.
    pub fn add_recent(&mut self, server_id: &str, path: &str, is_glob: bool) {
        let recent = &mut self.servers.entry(server_id.to_string()).or_default().recent;
        recent.retain(|r| !(r.path == path && r.is_glob == is_glob));
        recent.insert(0, RecentPath { path: path.to_string(), is_glob });
        recent.truncate(MAX_RECENT);
    }

    pub fn remove_recent(&mut self, server_id: &str, index: usize) {
        if let Some(server) = self.servers.get_mut(server_id) {
            if index < server.recent.len() {
                server.recent.remove(index);
            }
        }
    }

    pub fn clear_recent(&mut self, server_id: &str) {
        if let Some(server) = self.servers.get_mut(server_id) {
            server.recent.clear();
        }
    }

    /// A deleted Server takes its folder and Recent Paths with it.
    pub fn forget_server(&mut self, server_id: &str) {
        self.servers.remove(server_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty() {
        let state = PickerState::load(Path::new("/nonexistent/picker_state.json"));
        assert_eq!(state, PickerState::default());
        assert_eq!(state.sort.key, SortKey::Name);
        assert!(!state.sort.descending);
    }

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("picker-state-{}", std::process::id()));
        let path = dir.join("picker_state.json");
        let mut state = PickerState { sort: SortOrder { key: SortKey::Size, descending: true }, ..Default::default() };
        state.set_last_dir("srv-1", "/var/log/nginx");
        state.add_recent("srv-1", "/var/log/syslog", false);
        state.add_recent("srv-1", "/var/log/app/*.log", true);
        state.save_to(&path).unwrap();
        assert!(!dir.join("picker_state.json.tmp").exists());
        assert_eq!(PickerState::load(&path), state);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recent_newest_first_without_duplicates() {
        let mut state = PickerState::default();
        for p in ["/a", "/b", "/c", "/d", "/e"] {
            state.add_recent("s", p, false);
        }
        state.add_recent("s", "/b", false);
        let paths: Vec<&str> = state.recent("s").iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, ["/b", "/e", "/d", "/c", "/a"]);
    }

    #[test]
    fn recent_keeps_ten() {
        let mut state = PickerState::default();
        for i in 0..11 {
            state.add_recent("s", &format!("/f{i}"), false);
        }
        let recent = state.recent("s");
        assert_eq!(recent.len(), MAX_RECENT);
        assert_eq!(recent[0].path, "/f10");
        assert_eq!(recent[9].path, "/f1");
    }

    #[test]
    fn remove_and_clear() {
        let mut state = PickerState::default();
        state.add_recent("s", "/a", false);
        state.add_recent("s", "/b", false);
        state.remove_recent("s", 0);
        assert_eq!(state.recent("s").len(), 1);
        assert_eq!(state.recent("s")[0].path, "/a");
        state.clear_recent("s");
        assert!(state.recent("s").is_empty());
    }

    #[test]
    fn servers_are_separate_and_forgotten() {
        let mut state = PickerState::default();
        state.set_last_dir("web1", "/var/log/app");
        state.set_last_dir("db", "/var/lib/pgsql/log");
        state.add_recent("web1", "/x", false);
        assert_eq!(state.last_dir("web1"), Some("/var/log/app"));
        assert_eq!(state.last_dir("db"), Some("/var/lib/pgsql/log"));
        state.forget_server("web1");
        assert_eq!(state.last_dir("web1"), None);
        assert!(state.recent("web1").is_empty());
        assert_eq!(state.last_dir("db"), Some("/var/lib/pgsql/log"));
    }

    #[test]
    fn sort_toggle() {
        let order = SortOrder::default();
        let order = order.toggled(SortKey::Modified);
        assert_eq!(order, SortOrder { key: SortKey::Modified, descending: false });
        let order = order.toggled(SortKey::Modified);
        assert!(order.descending);
        let order = order.toggled(SortKey::Size);
        assert_eq!(order, SortOrder { key: SortKey::Size, descending: false });
    }
}
