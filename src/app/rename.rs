//! Logic: inline rename of a project, a Log Space or a scratch file in the sidebar.
//!
//! Only one row is renamed at a time. Enter (or losing focus) commits, Escape cancels.

use super::AppView;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

#[derive(Clone, PartialEq, Eq)]
pub enum RenameTarget {
    Project { id: String },
    LogSpace { project_id: String, space_id: String },
    /// A scratch file of a Log Space (Imported files are never renamed).
    File { project_id: String, space_id: String, file_id: String },
}

pub struct RenameState {
    pub target: RenameTarget,
    pub input: Entity<InputState>,
    /// Blur only commits once the input actually received focus.
    has_focused: bool,
    _subscription: Subscription,
}

impl AppView {
    /// Start renaming `target`, replacing its title with a focused, pre-selected input.
    pub fn begin_rename(
        &mut self,
        target: RenameTarget,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current_name));
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        let subscription = cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
            match event {
                InputEvent::PressEnter { .. } => this.commit_rename(cx),
                InputEvent::Focus => {
                    if let Some(state) = this.renaming.as_mut() {
                        state.has_focused = true;
                    }
                }
                InputEvent::Blur => {
                    if this.renaming.as_ref().is_some_and(|s| s.has_focused) {
                        this.commit_rename(cx);
                    }
                }
                _ => {}
            }
        });
        self.renaming = Some(RenameState {
            target,
            input,
            has_focused: false,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.renaming.take() else {
            return;
        };
        let name = state.input.read(cx).value().to_string();
        match &state.target {
            RenameTarget::Project { id } => self.rename_project(id, &name, cx),
            RenameTarget::LogSpace { project_id, space_id } => {
                self.rename_log_space(project_id, space_id, &name, cx)
            }
            RenameTarget::File { project_id, space_id, file_id } => {
                self.rename_scratch_file(project_id, space_id, file_id, &name, cx)
            }
        }
        cx.notify();
    }

    pub fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            cx.notify();
        }
    }
}
