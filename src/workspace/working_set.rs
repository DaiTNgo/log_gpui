use crate::column::ColumnLayoutState;
use crate::filter::FilterEngine;
use crate::search::SearchState;
use serde::{Deserialize, Serialize};

/// The target entity represented by a tab in the working set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TabTarget {
    /// An individual log file open for viewing.
    LogFile { file_id: String },
    /// An investigation case board showing chronological evidence trail.
    InvestigationBoard { case_id: String },
    /// A merged chronological view across multiple log files.
    ChronicleStream { file_ids: Vec<String> },
}

/// Independent per-tab session state. Each tab preserves its own complete
/// editor context so switching tabs never resets the user's place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSessionState {
    /// Active filter conditions for this tab.
    pub filter: FilterEngine,
    /// Active search query and match state.
    pub search: SearchState,
    /// Column widths, pin state, and resize state.
    pub column_layout: ColumnLayoutState,
    /// Horizontal scroll offset in pixels.
    pub scroll_x: f32,
    /// Vertical scroll offset in pixels.
    pub scroll_top: f32,
    /// Currently selected visible row index, if any.
    pub selected_row: Option<usize>,
}

impl TabSessionState {
    pub fn new() -> Self {
        Self {
            filter: FilterEngine::new(),
            search: SearchState::new(),
            column_layout: ColumnLayoutState::new(),
            scroll_x: 0.0,
            scroll_top: 0.0,
            selected_row: None,
        }
    }
}

impl Default for TabSessionState {
    fn default() -> Self {
        Self::new()
    }
}

/// A single open tab in the working set, combining its target identity
/// with the independent session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTab {
    /// What this tab is viewing.
    pub target: TabTarget,
    /// Per-tab editor session state.
    pub session: TabSessionState,
}

/// Deterministic headless state machine managing the ordered list of open tabs
/// and the currently active tab index. All working set operations are governed
/// here without any GPUI dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSet {
    tabs: Vec<WorkspaceTab>,
    active_index: Option<usize>,
}

impl Default for WorkingSet {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active_index: None,
        }
    }
}

impl WorkingSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of tabs in the working set.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Read-only slice of all open tabs.
    pub fn tabs(&self) -> &[WorkspaceTab] {
        &self.tabs
    }

    /// Mutable slice of all open tabs.
    pub fn tabs_mut(&mut self) -> &mut [WorkspaceTab] {
        &mut self.tabs
    }

    /// Index of the currently active tab, or None if the working set is empty.
    pub fn active_tab_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Reference to the currently active tab, if any.
    pub fn active_tab(&self) -> Option<&WorkspaceTab> {
        self.active_index.and_then(|i| self.tabs.get(i))
    }

    /// Mutable reference to the currently active tab, if any.
    pub fn active_tab_mut(&mut self) -> Option<&mut WorkspaceTab> {
        match self.active_index {
            Some(i) => self.tabs.get_mut(i),
            None => None,
        }
    }

    /// Open a tab for the given target. If a tab with the same target already
    /// exists, it is focused instead of creating a duplicate. Returns Some(index)
    /// if a new tab was created, None if an existing tab was focused.
    pub fn open_tab(&mut self, target: TabTarget, session: TabSessionState) -> Option<usize> {
        // Check for existing tab with same target
        if let Some(existing_idx) = self.tabs.iter().position(|t| t.target == target) {
            self.active_index = Some(existing_idx);
            return None;
        }

        let idx = self.tabs.len();
        self.tabs.push(WorkspaceTab { target, session });
        self.active_index = Some(idx);
        Some(idx)
    }

    /// Close a tab by index. Returns the closed tab if it existed.
    /// After closure, the active index is adjusted to keep focus on
    /// a valid tab (preferring the next tab, falling back to the previous).
    pub fn close_tab(&mut self, index: usize) -> Option<WorkspaceTab> {
        if index >= self.tabs.len() {
            return None;
        }

        let closed = self.tabs.remove(index);

        if self.tabs.is_empty() {
            self.active_index = None;
        } else if let Some(active) = self.active_index {
            if active == index {
                // Active tab was closed: prefer next, else previous
                self.active_index = Some(index.min(self.tabs.len() - 1));
            } else if active > index {
                // A tab before the active was closed: shift index down
                self.active_index = Some(active - 1);
            }
        }

        Some(closed)
    }

    /// Close the currently active tab.
    pub fn close_active_tab(&mut self) -> Option<WorkspaceTab> {
        match self.active_index {
            Some(idx) => self.close_tab(idx),
            None => None,
        }
    }

    /// Switch to a tab by index. No-op if index is out of range or the
    /// working set is empty.
    pub fn set_active_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_index = Some(index);
        }
    }

    /// Advance to the next tab, wrapping around from last to first.
    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let current = self.active_index.unwrap_or(0);
        self.active_index = Some((current + 1) % self.tabs.len());
    }

    /// Move to the previous tab, wrapping around from first to last.
    pub fn prev_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let current = self.active_index.unwrap_or(0);
        self.active_index = Some(if current == 0 {
            self.tabs.len() - 1
        } else {
            current - 1
        });
    }

    /// Select a tab by its 1-based keyboard shortcut number (Cmd+1 through Cmd+9).
    /// No-op if the number exceeds the tab count.
    pub fn select_tab_by_number(&mut self, number: usize) {
        if number == 0 || number > self.tabs.len() {
            return;
        }
        self.active_index = Some(number - 1);
    }

    /// Move a tab from one position to another, preserving session state.
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        // Update active index to follow the moved tab
        self.active_index = Some(to);
    }

    /// Check if a file is currently open in any tab.
    pub fn is_file_open(&self, file_id: &str) -> bool {
        self.tabs.iter().any(|t| {
            matches!(
                &t.target,
                TabTarget::LogFile { file_id: fid } if fid == file_id
            )
        })
    }

    /// Find the index of a tab by its target.
    pub fn find_tab(&self, target: &TabTarget) -> Option<usize> {
        self.tabs.iter().position(|t| t.target == *target)
    }
}