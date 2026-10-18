//! Logic: saving the session (debounced) so Log Spaces, tabs, Filters and
//! Markers survive a restart.

use super::AppView;
use gpui::Context;
use std::time::Duration;

/// How long to wait after the last change before writing to disk.
pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

impl AppView {
    /// Save the session after `SAVE_DEBOUNCE` without further changes.
    pub fn schedule_session_save(&mut self, cx: &mut Context<Self>) {
        if self.persistence.is_none() {
            return;
        }
        self.session_save_gen += 1;
        let generation = self.session_save_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            this.update(cx, |this, _cx| {
                if this.session_save_gen == generation {
                    this.save_session_now();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Write the session to disk now.
    pub fn save_session_now(&mut self) {
        self.persist_active_session();
        if let Some(persistence) = &self.persistence {
            if let Err(e) = persistence.save(&self.workspace_state) {
                eprintln!("Failed to save session: {e}");
            }
        }
    }
}
