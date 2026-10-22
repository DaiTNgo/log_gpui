//! Logic: Highlight Rules of the active Project. A rule paints every row
//! whose value in one column meets its condition; the paint of each row is
//! worked out again whenever the rules or the rows change.

use super::AppView;
use crate::highlight::{
    HighlightRule, MESSAGE_COLUMN, RuleError, move_rule, next_rule_id, paint_rows,
};
use crate::palette::{HighlightColor, next_rule_color};
use gpui::Context;
use std::sync::Arc;

impl AppView {
    pub fn highlight_rules(&self) -> &[HighlightRule] {
        self.active_project().map_or(&[], |p| p.highlight_rules.as_slice())
    }

    /// Work out the paint of every row of the open view. Only the Parsed View
    /// shows it.
    pub fn recompute_highlight_paints(&mut self) {
        let rules = self.highlight_rules().to_vec();
        self.highlight_paints = Arc::new(paint_rows(&rules, &self.all_entries));
        self.rule_counts = rules
            .iter()
            .map(|r| {
                let n = crate::highlight::count_matches(r, &self.all_entries, &self.visible_indices);
                (r.id, n)
            })
            .collect();
    }

    /// Paint the rows from `from` on, appended after the rows painted, and
    /// count them in the rules' counts. False when the paints did not cover
    /// the rows before `from` (the caller paints everything again).
    pub(crate) fn extend_highlight_paints(&mut self, from: usize) -> bool {
        if self.highlight_paints.len() != from {
            return false;
        }
        let rules = self.highlight_rules().to_vec();
        let new = crate::highlight::paint_rows_from(&rules, &self.all_entries, from);
        Arc::make_mut(&mut self.highlight_paints).extend(new);
        for rule in &rules {
            let n = crate::highlight::count_matches(rule, &self.all_entries, &self.visible_indices[from..]);
            *self.rule_counts.entry(rule.id).or_insert(0) += n;
        }
        true
    }

    fn rules_changed(&mut self, cx: &mut Context<Self>) {
        self.recompute_highlight_paints();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Add a rule at the end of the list (lowest priority).
    pub fn add_highlight_rule(
        &mut self,
        column: &str,
        values: &[String],
        custom: &[String],
        color: HighlightColor,
        cx: &mut Context<Self>,
    ) -> Result<(), RuleError> {
        let Some(project) = self.active_project_mut() else { return Ok(()) };
        let id = next_rule_id(&project.highlight_rules);
        let rule = HighlightRule::new(id, column, values, custom, color.clone())?;
        project.highlight_rules.push(rule);
        project.recent_colors.push(color);
        self.rules_changed(cx);
        Ok(())
    }

    /// "Highlight rows where <column> = <value>" from the menu of a cell: a
    /// normal rule with the next color of the palette. On MESSAGE the text is a
    /// Custom Value.
    pub fn highlight_rows_where(&mut self, column: &str, value: &str, cx: &mut Context<Self>) {
        let color = next_rule_color(self.highlight_rules().len());
        let (values, custom) = if column == MESSAGE_COLUMN {
            (Vec::new(), vec![value.to_string()])
        } else {
            (vec![value.to_string()], Vec::new())
        };
        let _ = self.add_highlight_rule(column, &values, &custom, color, cx);
    }

    pub fn remove_highlight_rule(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            project.highlight_rules.retain(|r| r.id != id);
        }
        self.rules_changed(cx);
    }

    pub fn toggle_highlight_rule(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(rule) = self.active_project_mut().and_then(|p| p.highlight_rules.iter_mut().find(|r| r.id == id)) {
            rule.enabled = !rule.enabled;
        }
        self.rules_changed(cx);
    }

    pub fn set_highlight_rule_color(&mut self, id: u64, color: HighlightColor, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            if let Some(rule) = project.highlight_rules.iter_mut().find(|r| r.id == id) {
                rule.color = color.clone();
            }
            project.recent_colors.push(color);
        }
        self.rules_changed(cx);
    }

    /// Move a rule to a new place in the priority order.
    pub fn move_highlight_rule(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            move_rule(&mut project.highlight_rules, from, to);
        }
        self.rules_changed(cx);
    }
}

/// The "+ Add rule" form while it is open.
#[derive(Debug, Clone)]
pub struct RuleForm {
    /// Column key chosen, if any.
    pub column: Option<String>,
    /// Ticked Filter Values, as shown (case folded when the rule is made).
    pub values: Vec<String>,
    pub custom: Vec<String>,
    pub color: HighlightColor,
    pub error: Option<&'static str>,
}

impl AppView {
    /// The columns a rule can use, in the order of the Filter: every
    /// Filterable Column (the Source Column included) and MESSAGE. Never
    /// TIMESTAMP.
    pub fn rule_columns(&self) -> Vec<(String, String)> {
        let mut columns: Vec<(String, String)> = self
            .filter_index
            .columns_in_use()
            .map(|c| (c.key.clone(), c.label.clone()))
            .collect();
        columns.push((MESSAGE_COLUMN.to_string(), "Message".to_string()));
        columns
    }

    pub fn open_rule_form(&mut self, cx: &mut Context<Self>) {
        let color = next_rule_color(self.highlight_rules().len());
        self.rule_form = Some(RuleForm { column: None, values: Vec::new(), custom: Vec::new(), color, error: None });
        cx.notify();
    }

    pub fn cancel_rule_form(&mut self, cx: &mut Context<Self>) {
        self.rule_form = None;
        cx.notify();
    }

    pub fn rule_form_pick_column(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            form.column = Some(key.to_string());
            form.values.clear();
            form.custom.clear();
            form.error = None;
        }
        cx.notify();
    }

    pub fn rule_form_toggle_value(&mut self, value: &str, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            match form.values.iter().position(|v| v == value) {
                Some(pos) => {
                    form.values.remove(pos);
                }
                None => form.values.push(value.to_string()),
            }
            form.error = None;
        }
        cx.notify();
    }

    /// Add a Custom Value to the form. Blank or repeated text is ignored.
    pub fn rule_form_add_custom(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        let text = text.trim();
        let Some(form) = &mut self.rule_form else { return false };
        if text.is_empty() || form.custom.iter().any(|c| c == text) {
            return false;
        }
        form.custom.push(text.to_string());
        form.error = None;
        cx.notify();
        true
    }

    pub fn rule_form_remove_custom(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            form.custom.retain(|c| c != text);
        }
        cx.notify();
    }

    pub fn rule_form_set_color(&mut self, color: HighlightColor, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            form.color = color;
        }
        cx.notify();
    }

    /// Make the rule: it goes at the end of the list.
    pub fn rule_form_submit(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.rule_form.clone() else { return };
        let Some(column) = form.column.clone() else {
            if let Some(f) = &mut self.rule_form {
                f.error = Some("Choose a column");
            }
            cx.notify();
            return;
        };
        match self.add_highlight_rule(&column, &form.values, &form.custom, form.color.clone(), cx) {
            Ok(()) => self.rule_form = None,
            Err(e) => {
                if let Some(f) = &mut self.rule_form {
                    f.error = Some(match e {
                        RuleError::TimestampColumn => "Timestamp cannot be highlighted",
                        RuleError::MessageNeedsCustomValue => "Message takes added values only",
                        RuleError::NoCondition => "Tick a value or add one",
                    });
                }
            }
        }
        cx.notify();
    }
}
