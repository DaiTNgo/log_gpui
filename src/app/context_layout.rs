//! Logic: the size of the context sidebar (its width, the Inspector's
//! height under the splitter), the line naming what it acts on, and the
//! Time Range presets of its Filter section.

use super::AppView;
use crate::workspace::TabTarget;
use crate::workspace::state::{CONTEXT_SIDEBAR_WIDTH, InspectorMode};
use crate::workspace::working_set::ViewMode;
use gpui::{Context, Pixels};

/// The Time Range presets: label and how far back from the newest line.
pub const TIME_PRESETS: [(&str, Option<i64>); 4] =
    [("5m", Some(5 * 60)), ("1h", Some(3600)), ("24h", Some(86_400)), ("All", None)];

/// The From bound of a preset `secs` back from `latest` (seconds, on the
/// file's own clock), written as the Timestamp column writes a time.
pub fn preset_from(latest: f64, secs: i64) -> Option<String> {
    let from = latest.floor() as i64 - secs;
    let dt = chrono::DateTime::from_timestamp(from, 0)?.naive_utc();
    Some(dt.format("%Y-%m-%d %H:%M:%S").to_string())
}

impl AppView {
    /// What the context sidebar acts on: `api.log · Parsed`.
    pub fn context_scope_label(&self) -> String {
        let project = self.active_project();
        let name = match self.active_target() {
            Some(TabTarget::LogSpace { space_id }) => {
                project.and_then(|p| p.log_space(&space_id)).map_or(space_id.clone(), |s| s.name.clone())
            }
            Some(TabTarget::FileView { space_id, file_id }) => project
                .and_then(|p| p.log_space(&space_id))
                .and_then(|s| s.file(&file_id))
                .map_or(file_id.clone(), |f| f.display_name.clone()),
            _ => String::new(),
        };
        let view = match self.view_mode {
            ViewMode::Parsed => "Parsed View",
            ViewMode::Raw => "Raw View",
        };
        if name.is_empty() { view.to_string() } else { format!("{name} · {view}") }
    }

    /// The splitter above the Inspector was dragged to window height `y`.
    pub fn drag_inspector_split(&mut self, y: Pixels, cx: &mut Context<Self>) {
        let body = self.context_body_bounds.get();
        let total = f32::from(body.size.height);
        let bottom = f32::from(body.origin.y) + total;
        self.workspace_state.context_layout.set_inspector_height(bottom - f32::from(y), total);
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// The sidebar's left edge was dragged to window position `x`.
    pub fn drag_context_width(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let body = self.context_body_bounds.get();
        let right = f32::from(body.origin.x + body.size.width);
        if right > 0.0 {
            self.workspace_state.context_layout.set_width(right - f32::from(x));
            self.schedule_session_save(cx);
            cx.notify();
        }
    }

    pub fn reset_context_width(&mut self, cx: &mut Context<Self>) {
        self.workspace_state.context_layout.set_width(CONTEXT_SIDEBAR_WIDTH);
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn set_inspector_mode(&mut self, mode: InspectorMode, cx: &mut Context<Self>) {
        self.workspace_state.context_layout.inspector_mode = mode;
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// A Time Range preset: From the newest line of the view less `secs`,
    /// To empty; `None` clears the Time Range. Worked out once, when chosen.
    pub fn apply_time_preset(&mut self, secs: Option<i64>, cx: &mut Context<Self>) {
        let from = match secs {
            Some(secs) => match self.filter_index.latest_timestamp().and_then(|t| preset_from(t, secs)) {
                Some(from) => from,
                None => return,
            },
            None => String::new(),
        };
        self.filter_state.set_time_range(&from, "");
        self.filter_ui.inputs_dirty = true;
        self.apply_filter_change(cx);
    }
}

/// A column of the line the Inspector shows.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectorField {
    pub key: String,
    pub label: String,
    pub value: String,
}

impl AppView {
    /// The columns with a value of the current line's Parent Line, in the
    /// Parsed View: TIMESTAMP, the columns the view detected, MESSAGE.
    /// Empty when no column could be read from it.
    pub fn inspector_fields(&self) -> Vec<InspectorField> {
        if self.view_mode != ViewMode::Parsed {
            return Vec::new();
        }
        let Some(mut ix) = self.current_line().and_then(|l| self.entry_of_line_ref(&l)) else { return Vec::new() };
        let rows = self.rows();
        // A Continuation Line shows the fields of its Parent Line.
        while ix > 0 && rows.get(ix).is_some_and(|e| e.is_continuation()) {
            ix -= 1;
        }
        let Some(entry) = rows.get(ix) else { return Vec::new() };
        if entry.is_continuation() {
            return Vec::new();
        }
        let mut fields = Vec::new();
        let time = entry.time_str();
        if !time.is_empty() {
            fields.push(InspectorField { key: "timestamp".into(), label: "Timestamp".into(), value: time.to_string() });
        }
        for col in self.filter_index.columns_in_use() {
            if let Some(v) = crate::filter_index::column_value(&entry, &col.key) {
                fields.push(InspectorField { key: col.key.clone(), label: col.label.clone(), value: v.to_string() });
            }
        }
        if fields.is_empty() {
            return fields;
        }
        if let Some(v) = crate::filter_index::column_value(&entry, crate::columns::MESSAGE_KEY) {
            fields.push(InspectorField { key: crate::columns::MESSAGE_KEY.into(), label: "Message".into(), value: v.to_string() });
        }
        fields
    }

    /// The Inspector's ↑ / ↓: the row shown before or after the current one.
    pub fn inspector_step(&mut self, down: bool, cx: &mut Context<Self>) {
        use crate::go_to::RowKey;
        if self.view_mode == ViewMode::Parsed {
            self.move_row_by_key(if down { RowKey::Down } else { RowKey::Up }, cx);
        }
    }

    /// Copy what the Inspector shows: the stored text, or the fields as `name: value` lines.
    pub fn copy_inspector(&mut self, cx: &mut Context<Self>) {
        let fields = self.inspector_fields();
        let text = if self.workspace_state.context_layout.inspector_mode == InspectorMode::Fields && !fields.is_empty() {
            fields.iter().map(|f| format!("{}: {}", f.label, f.value)).collect::<Vec<_>>().join("\n")
        } else {
            match self.inspector_text.clone() {
                Some(t) => t,
                None => return,
            }
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    }
}
