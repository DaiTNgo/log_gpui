//! Warm Tabs (memory-budget, group 6): the files of a tab the user left
//! keep their Line Index while every such file fits in the Warm Tab Budget,
//! so choosing the tab again shows it at once. Past the budget the files
//! left longest ago go first, and their tabs are Dormant.
//!
//! A file is warm, not a tab: a file can be shown by a Log Space and by its
//! File View, and what is kept (`AppView::parsed_files`) is by `file_id`.

use std::collections::HashMap;

/// Default Warm Tab Budget, in MB.
pub const DEFAULT_WARM_TAB_BUDGET_MB: u32 = 512;

/// The files kept for the tabs left: when each was left and the memory it
/// holds.
#[derive(Debug, Default)]
pub struct WarmFiles {
    files: HashMap<String, (u64, usize)>,
    /// Bumped at each leave: the files left together are as old.
    tick: u64,
}

impl WarmFiles {
    /// The files just left, with the memory each holds. One kept already
    /// counts as left now. A file counts as a byte at least, so a budget of
    /// 0 keeps none, an empty one neither.
    pub fn leave(&mut self, files: impl IntoIterator<Item = (String, usize)>) {
        self.tick += 1;
        for (id, bytes) in files {
            self.files.insert(id, (self.tick, bytes.max(1)));
        }
    }

    /// Take `file_id` out: it is shown again. Whether it was kept.
    pub fn take(&mut self, file_id: &str) -> bool {
        self.files.remove(file_id).is_some()
    }

    pub fn contains(&self, file_id: &str) -> bool {
        self.files.contains_key(file_id)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The memory every file kept holds.
    pub fn total(&self) -> usize {
        self.files.values().map(|(_, b)| b).sum()
    }

    /// Let go of the files left longest ago until the rest fit in `budget`
    /// bytes: the ones let go, oldest first. Files left together go
    /// together.
    pub fn fit(&mut self, budget: usize) -> Vec<String> {
        let mut gone = Vec::new();
        while self.total() > budget {
            let Some(oldest) = self.files.values().map(|(t, _)| *t).min() else { break };
            let mut ids: Vec<String> = self.files.iter().filter(|(_, (t, _))| *t == oldest).map(|(id, _)| id.clone()).collect();
            ids.sort();
            for id in &ids {
                self.files.remove(id);
            }
            gone.extend(ids);
        }
        gone
    }
}
