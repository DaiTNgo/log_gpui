//! Logic: reading the Detected Fields of the open view's lines. A `key=value`
//! field is read only while its switch in the Columns section is on; when
//! that changes, the open view's lines are read again in the background and
//! its Filter columns detected again. New live lines are read as they come.

use super::AppView;
use crate::extract::LineConfig;
use gpui::{AppContext as _, Context};
use std::sync::Arc;

/// The `LineConfig` in use and the fields it was compiled from, to compile
/// again only when they change.
#[derive(Default)]
pub struct LineCache {
    /// The `key=value` Detected Fields turned on it was compiled with.
    kv: Vec<String>,
    config: Arc<LineConfig>,
    /// Bumped by every change; a background read of an older one is dropped.
    pub generation: u64,
    /// Whether the open view's lines are being read again.
    pub reading: bool,
}

/// Why a column a Filter condition is on is not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnreadReason {
    /// It is a `key=value` Detected Field turned off.
    DetectedOff,
    /// No line of the view has it.
    NotRead,
}

impl UnreadReason {
    pub fn text(&self) -> &'static str {
        match self {
            UnreadReason::DetectedOff => "column turned off",
            UnreadReason::NotRead => "no line has it",
        }
    }

    pub fn can_turn_on(&self) -> bool {
        !matches!(self, UnreadReason::NotRead)
    }
}

impl AppView {
    /// Why the column `key` is not read, when the view has no such column.
    pub fn unread_column(&self, key: &str) -> Option<UnreadReason> {
        if self.filter_index.column(key).is_some() {
            return None;
        }
        if self.filter_index.kv_counts().contains_key(&crate::model::FieldKey::new(key)) {
            return Some(UnreadReason::DetectedOff);
        }
        Some(UnreadReason::NotRead)
    }

    /// The Focus Pane's notice when a Filter condition on a column no longer
    /// read is why no line passes.
    pub(crate) fn unread_filter_notice(&self) -> Option<crate::focus::FocusPaneNotice> {
        self.filter_state.columns.iter().filter(|c| c.is_active() && !c.hide).find_map(|c| {
            let reason = self.unread_column(&c.column)?;
            (reason != UnreadReason::NotRead).then(|| crate::focus::FocusPaneNotice::UnreadColumn {
                key: c.column.clone(),
                name: crate::columns::default_name(&c.column).to_string(),
                reason: reason.text().to_string(),
                can_turn_on: reason.can_turn_on(),
            })
        })
    }

    /// "Turn column back on": switch a Detected Field back on.
    pub fn turn_column_back_on(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(UnreadReason::DetectedOff) = self.unread_column(key) {
            self.set_column_shown(key, true, cx);
        }
    }

    /// How lines are read beyond Auto-detect: the `key=value` Detected Fields
    /// whose switch is on.
    pub(crate) fn line_config(&mut self) -> Arc<LineConfig> {
        // Every Field Column turned on may be a `key=value` Detected Field;
        // one the line has already (a JSON key, a Log Format's) is not read
        // again.
        let settings = self.column_settings();
        let kv = crate::columns::pane_fields(settings.pane.into_iter().filter(|(_, on)| *on).map(|(k, _)| k));
        if self.line_cache.kv != kv {
            self.line_cache.config = Arc::new(LineConfig::new(&kv));
            self.line_cache.kv = kv;
        }
        self.line_cache.config.clone()
    }

    /// Read the view's lines again if the fields read beyond Auto-detect
    /// changed (a Detected Field turned on or off).
    pub(crate) fn reread_if_config_changed(&mut self, cx: &mut Context<Self>) {
        let before = self.line_cache.config.clone();
        if !Arc::ptr_eq(&before, &self.line_config()) {
            self.detected_fields_changed(cx);
        }
    }

    /// Read the Detected Fields of the rows just put in the view, and detect
    /// their Filter columns.
    pub(crate) fn read_detected_fields(&mut self) {
        let config = self.line_config();
        if config.is_empty() && self.all_entries.iter().all(|e| e.extra.is_empty()) {
            return;
        }
        config.apply(&mut self.all_entries);
        self.after_detected_fields_read();
    }

    /// The fields of a new live line.
    pub(crate) fn read_detected_fields_of(&mut self, entry: &mut crate::model::LogEntry) {
        let config = self.line_config();
        if !config.is_empty() {
            entry.extra = config.extra_spans(entry);
        }
    }

    fn after_detected_fields_read(&mut self) {
        self.filter_index.rebuild_fields(&self.all_entries);
        let fitted = crate::column::FieldMaxChars::scan_entries(&self.all_entries);
        self.column_layout.max_chars.fields = fitted.fields;
    }

    /// Read the open view's lines again with the changed fields, in the
    /// background; the view stays usable meanwhile.
    pub(crate) fn detected_fields_changed(&mut self, cx: &mut Context<Self>) {
        self.schedule_session_save(cx);
        let config = self.line_config();
        self.line_cache.generation += 1;
        let generation = self.line_cache.generation;
        let rows = self.all_entries.len();
        let lines = crate::extract::snapshot(&self.all_entries);
        self.line_cache.reading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let read = cx.background_spawn(async move { config.read_snapshot(&lines) }).await;
            this.update(cx, |this, cx| {
                if this.line_cache.generation != generation {
                    return;
                }
                this.line_cache.reading = false;
                if this.all_entries.len() == rows {
                    for (entry, extra) in this.all_entries.iter_mut().zip(read) {
                        entry.extra = extra;
                    }
                    this.after_detected_fields_read();
                } else {
                    // Live lines came meanwhile: read them all now.
                    this.read_detected_fields();
                }
                this.apply_filters_and_refresh(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
