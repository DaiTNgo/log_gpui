//! Logic: the size of the context sidebar (its width, the Inspector's
//! height under the splitter) and the Inspector's fields.

use super::AppView;
use crate::workspace::state::{CONTEXT_SIDEBAR_WIDTH, InspectorMode};
use crate::workspace::working_set::ViewMode;
use gpui::{Context, Pixels};

impl AppView {
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
        for col in self.filter_index.read(&self.view.rows).columns_in_use() {
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
