use crate::column::ColumnLayoutState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The target entity represented by a tab in the working set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TabTarget {
    /// A Log Space: its files interleaved by timestamp.
    LogSpace { space_id: String },
    /// One file of a Log Space opened on its own.
    FileView { space_id: String, file_id: String },
    /// An investigation case board showing chronological evidence trail.
    InvestigationBoard { case_id: String },
}

impl TabTarget {
    /// The Log Space this tab views, if it is a Log Space or File View tab.
    pub fn space_id(&self) -> Option<&str> {
        match self {
            TabTarget::LogSpace { space_id } | TabTarget::FileView { space_id, .. } => {
                Some(space_id)
            }
            TabTarget::InvestigationBoard { .. } => None,
        }
    }

    /// Whether this tab shows log lines (and so has Filter, Search, Markers).
    pub fn is_log_view(&self) -> bool {
        !matches!(self, TabTarget::InvestigationBoard { .. })
    }
}

/// Whether a log tab shows the parsed columns or each file's raw text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewMode {
    Raw,
    #[default]
    Parsed,
}

/// Independent per-tab session state. Each tab preserves its own complete
/// editor context so switching tabs never resets the user's place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSessionState {
    /// Column widths, pin state, and resize state.
    pub column_layout: ColumnLayoutState,
    /// Horizontal scroll offset in pixels.
    pub scroll_x: f32,
    /// Vertical scroll offset in pixels.
    pub scroll_top: f32,
    /// Currently selected visible row index, if any.
    pub selected_row: Option<usize>,
    /// Raw View or Parsed View.
    #[serde(default)]
    pub mode: ViewMode,
    /// `file_id`s of the File Sections collapsed in the Raw View.
    #[serde(default)]
    pub collapsed_sections: HashSet<String>,
}

impl TabSessionState {
    pub fn new() -> Self {
        Self {
            column_layout: ColumnLayoutState::new(),
            scroll_x: 0.0,
            scroll_top: 0.0,
            selected_row: None,
            mode: ViewMode::Parsed,
            collapsed_sections: HashSet::new(),
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
    /// A Pinned Tab sits before all other tabs and survives the bulk closes.
    #[serde(default)]
    pub pinned: bool,
}

/// Which tabs a bulk close reaches, relative to the clicked tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseScope {
    Others,
    Left,
    Right,
    All,
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
        self.tabs.push(WorkspaceTab { target, session, pinned: false });
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

    /// Whether the tab at `index` is a Pinned Tab.
    pub fn is_pinned(&self, index: usize) -> bool {
        self.tabs.get(index).is_some_and(|t| t.pinned)
    }

    fn pinned_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.pinned).count()
    }

    /// Move the tab at `from` to `to`, keeping the active tab the same tab.
    fn relocate(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        let active = self.active_index.map(|a| {
            if a == from {
                to
            } else if from < a && a <= to {
                a - 1
            } else if to <= a && a < from {
                a + 1
            } else {
                a
            }
        });
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active_index = active;
    }

    /// Pin the tab: it moves behind the other Pinned Tabs. Returns its new index.
    pub fn pin_tab(&mut self, index: usize) -> usize {
        if index >= self.tabs.len() || self.tabs[index].pinned {
            return index;
        }
        let to = self.pinned_count();
        self.tabs[index].pinned = true;
        self.relocate(index, to);
        to
    }

    /// Unpin the tab: it moves to the front of the unpinned tabs. Returns its new index.
    pub fn unpin_tab(&mut self, index: usize) -> usize {
        if !self.is_pinned(index) {
            return index;
        }
        self.tabs[index].pinned = false;
        let to = self.pinned_count();
        self.relocate(index, to);
        to
    }

    fn in_scope(scope: CloseScope, clicked: usize, i: usize) -> bool {
        match scope {
            CloseScope::Others => i != clicked,
            CloseScope::Left => i < clicked,
            CloseScope::Right => i > clicked,
            CloseScope::All => true,
        }
    }

    /// How many tabs `close_scope` would close: those in scope that are not pinned.
    pub fn closable_count(&self, scope: CloseScope, clicked: usize) -> usize {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(i, t)| !t.pinned && Self::in_scope(scope, clicked, *i))
            .count()
    }

    /// Close the unpinned tabs in `scope` relative to the clicked tab. The
    /// active tab stays when it survives; otherwise the clicked tab (or, for
    /// Close All, the first remaining tab) becomes active.
    pub fn close_scope(&mut self, scope: CloseScope, clicked: usize) {
        let keep: Vec<bool> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| t.pinned || !Self::in_scope(scope, clicked, i))
            .collect();
        let new_index = |old: usize| keep[..old].iter().filter(|k| **k).count();
        let active = self.active_index;
        let next_active = match active {
            Some(a) if keep.get(a) == Some(&true) => Some(new_index(a)),
            _ if clicked < keep.len() && keep[clicked] => Some(new_index(clicked)),
            _ => keep.iter().position(|k| *k).map(new_index),
        };
        let mut flags = keep.into_iter();
        self.tabs.retain(|_| flags.next().unwrap_or(true));
        self.active_index = if self.tabs.is_empty() { None } else { next_active };
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

    /// Check if a file is currently open in a File View tab.
    pub fn is_file_open(&self, file_id: &str) -> bool {
        self.tabs.iter().any(|t| {
            matches!(
                &t.target,
                TabTarget::FileView { file_id: fid, .. } if fid == file_id
            )
        })
    }

    /// Close every tab that views the Log Space (its Log Space tab and all
    /// its File Views).
    pub fn close_tabs_for_space(&mut self, space_id: &str) {
        while let Some(i) = self
            .tabs
            .iter()
            .position(|t| t.target.space_id() == Some(space_id))
        {
            self.close_tab(i);
        }
    }

    /// Close the File View showing the given file.
    pub fn close_tabs_for_file(&mut self, space_id: &str, file_id: &str) {
        if let Some(i) = self.find_tab(&TabTarget::FileView {
            space_id: space_id.to_string(),
            file_id: file_id.to_string(),
        }) {
            self.close_tab(i);
        }
    }

    /// Find the index of a tab by its target.
    pub fn find_tab(&self, target: &TabTarget) -> Option<usize> {
        self.tabs.iter().position(|t| t.target == *target)
    }
}