//! Logic: project switcher, creation, removal and rename.

use super::AppView;
use crate::workspace::project::ProjectColorAccent;
use gpui::{
    Context,
};

impl AppView {
    // ── Issue 02: Project Switcher ──────────────────────────────────────────

    pub fn toggle_project_switcher(&mut self, cx: &mut Context<Self>) {
        self.project_switcher_open = !self.project_switcher_open;
        if self.project_switcher_open {
            self.project_switcher_index = self
                .workspace_state
                .active_project_id
                .as_deref()
                .and_then(|id| self.workspace_state.project_index(id))
                .unwrap_or(0);
            self.project_renaming = None;
            self.project_rename_buffer.clear();
            // Closing the palette keeps the UI uncluttered while switching.
            self.is_palette_open = false;
        }
        cx.notify();
    }

    pub fn switch_project_to(&mut self, project_id: &str, cx: &mut Context<Self>) {
        if self.active_project().map(|p| p.id.as_str()) == Some(project_id) {
            self.project_switcher_open = false;
            self.project_renaming = None;
            cx.notify();
            return;
        }
        self.persist_active_session();
        if self.workspace_state.switch_project(project_id).is_none() {
            self.project_switcher_open = false;
            cx.notify();
            return;
        }
        self.restore_active_session(cx);
        self.project_switcher_open = false;
        self.project_switcher_index = self
            .workspace_state
            .project_index(project_id)
            .unwrap_or(0);
        self.project_renaming = None;
        self.project_rename_buffer.clear();
        cx.notify();
    }

    pub fn create_new_project(&mut self, cx: &mut Context<Self>) {
        self.persist_active_session();
        let base = "New Project";
        let mut name = base.to_string();
        if self.workspace_state.project_named(&name) {
            let mut suffix = self.workspace_state.project_count() + 1;
            loop {
                name = format!("{} {}", base, suffix);
                if !self.workspace_state.project_named(&name) {
                    break;
                }
                suffix += 1;
            }
        }
        let color =
            ProjectColorAccent::ALL[self.workspace_state.project_count() % ProjectColorAccent::ALL.len()];
        self.workspace_state
            .create_project(&name, color);
        self.restore_active_session(cx);
        self.project_switcher_open = false;
        self.project_renaming = None;
        self.project_rename_buffer.clear();
        cx.notify();
    }

    pub fn remove_project_at(&mut self, project_id: &str, cx: &mut Context<Self>) {
        if self.workspace_state.project_count() <= 1 {
            return;
        }
        let removing_active = self.active_project().map(|p| p.id.as_str()) == Some(project_id);
        self.persist_active_session();
        let Some(removed) = self.workspace_state.remove_project(project_id) else { return };
        for file in removed.log_spaces.iter().flat_map(|s| s.files.iter()) {
            self.delete_copy_of(file);
        }
        if removing_active {
            self.restore_active_session(cx);
        }
        let count = self.workspace_state.project_count();
        if self.project_switcher_index >= count {
            self.project_switcher_index = count.saturating_sub(1);
        }
        if self.project_switcher_open {
            cx.notify();
        }
    }

    /// Rename a project (ignores blank names).
    pub fn rename_project(&mut self, project_id: &str, name: &str, cx: &mut Context<Self>) {
        let name = name.trim();
        if !name.is_empty() {
            self.workspace_state.rename_project(project_id, name);
        }
        cx.notify();
    }

    pub fn begin_project_rename(&mut self, project_id: &str, cx: &mut Context<Self>) {
        self.project_renaming = Some(project_id.to_string());
        self.project_rename_buffer.clear();
        cx.notify();
    }

    /// Commit the pending rename buffer for the project currently being renamed.
    pub(crate) fn commit_project_rename(&mut self, cx: &mut Context<Self>) {
        let name = self.project_rename_buffer.trim().to_string();
        if let Some(id) = self.project_renaming.take() {
            if !name.is_empty() {
                self.workspace_state.rename_project(&id, &name);
            }
        }
        self.project_rename_buffer.clear();
        cx.notify();
    }

    /// Feed a single character into the rename buffer (app-level key capture).
    pub(crate) fn project_rename_push_char(&mut self, key: &str, cx: &mut Context<Self>) {
        if key.chars().count() == 1 && !self.project_rename_buffer.contains('\n') {
            self.project_rename_buffer.push_str(key);
            cx.notify();
        }
    }
}
