//! Logic: Extraction Rules of the active Project. A rule reads a Field Column
//! from every line; when the rules change, the open view's lines are read
//! again in the background and its Filter columns detected again. New live
//! lines are read with the rules as they come.

use super::AppView;
use crate::extract::{ExtractionRule, LineConfig, RuleScope, next_rule_id};
use gpui::{AppContext as _, Context};
use std::sync::Arc;

/// The rules a `LineConfig` was compiled from, to compile again only when
/// they change.
#[derive(Default)]
pub struct RuleCache {
    rules: Vec<ExtractionRule>,
    /// The `key=value` Detected Fields turned on it was compiled with.
    kv: Vec<String>,
    config: Arc<LineConfig>,
    /// Bumped by every change; a background read of an older one is dropped.
    pub generation: u64,
    /// Whether the open view's lines are being read again.
    pub reading: bool,
    /// Rules deleted in this session, for "Turn column back on".
    pub removed: Vec<ExtractionRule>,
}

/// Why a column a Filter condition is on is not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnreadReason {
    /// Its Extraction Rule is turned off.
    RuleOff(u64),
    /// Its Extraction Rule was deleted in this session (it can come back).
    RuleDeleted,
    /// It is a `key=value` Detected Field turned off.
    DetectedOff,
    /// No line of the view has it.
    NotRead,
}

impl UnreadReason {
    pub fn text(&self) -> &'static str {
        match self {
            UnreadReason::RuleOff(_) => "rule turned off",
            UnreadReason::RuleDeleted => "rule deleted",
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
        let rules: Vec<&ExtractionRule> = self.extraction_rules().iter().filter(|r| r.name == key).collect();
        if let Some(off) = rules.iter().find(|r| !r.enabled).filter(|_| rules.iter().all(|r| !r.enabled)) {
            return Some(UnreadReason::RuleOff(off.id));
        }
        if rules.is_empty() && self.rule_cache.removed.iter().any(|r| r.name == key) {
            return Some(UnreadReason::RuleDeleted);
        }
        if rules.is_empty() && self.filter_index.kv_counts().contains_key(&crate::model::FieldKey::new(key)) {
            return Some(UnreadReason::DetectedOff);
        }
        Some(UnreadReason::NotRead)
    }

    /// The Focus Pane's notice when a Filter condition on a column no longer
    /// read is why no line passes.
    pub(crate) fn unread_filter_notice(&self) -> Option<crate::focus::FocusPaneNotice> {
        self.filter_state.columns.iter().filter(|c| c.is_active() && !c.hide).find_map(|c| {
            let reason = self.unread_column(&c.column)?;
            (reason != UnreadReason::NotRead || self.column_was_read(&c.column)).then(|| {
                crate::focus::FocusPaneNotice::UnreadColumn {
                    key: c.column.clone(),
                    name: self.column_display_name(&c.column),
                    reason: reason.text().to_string(),
                    can_turn_on: reason.can_turn_on(),
                }
            })
        })
    }

    /// Whether `key` is a column the Project reads or read (a rule's field).
    fn column_was_read(&self, key: &str) -> bool {
        self.extraction_rules().iter().any(|r| r.name == key)
            || self.rule_cache.removed.iter().any(|r| r.name == key)
            || self.filter_index.kv_counts().contains_key(&crate::model::FieldKey::new(key))
    }

    /// "Turn column back on": turn its rule on again, or bring back the rule
    /// deleted in this session.
    pub fn turn_column_back_on(&mut self, key: &str, cx: &mut Context<Self>) {
        match self.unread_column(key) {
            Some(UnreadReason::RuleOff(id)) => self.set_extraction_rule_enabled(id, true, cx),
            Some(UnreadReason::DetectedOff) => self.set_column_shown(key, true, cx),
            Some(UnreadReason::RuleDeleted) => {
                let Some(at) = self.rule_cache.removed.iter().rposition(|r| r.name == key) else { return };
                let mut rule = self.rule_cache.removed.remove(at);
                let Some(project) = self.active_project_mut() else { return };
                rule.id = next_rule_id(&project.extraction_rules);
                rule.enabled = true;
                project.columns.set_shown(&rule.name, true);
                project.extraction_rules.push(rule);
                self.extraction_rules_changed(cx);
            }
            _ => {}
        }
    }

    pub fn extraction_rules(&self) -> &[ExtractionRule] {
        self.active_project().map_or(&[], |p| p.extraction_rules.as_slice())
    }

    /// The compiled rules of the active Project.
    pub(crate) fn line_config(&mut self) -> Arc<LineConfig> {
        let rules = self.extraction_rules().to_vec();
        // Every Field Column turned on may be a `key=value` Detected Field;
        // one the line has already (a JSON key, a rule's) is not read again.
        let settings = self.column_settings();
        let on_in_pane = settings.pane.iter().filter(|(_, on)| **on).map(|(k, _)| k.clone());
        let mut kv: Vec<String> = settings
            .fields
            .iter()
            .cloned()
            .chain(on_in_pane)
            .filter(|f| !rules.iter().any(|r| r.name == *f))
            .filter(|f| crate::model::is_field_name(f) && !crate::columns::BUILT_IN.iter().any(|(k, _)| k == f))
            .collect();
        let mut seen = std::collections::HashSet::new();
        kv.retain(|f| seen.insert(f.clone()));
        if self.rule_cache.rules != rules || self.rule_cache.kv != kv {
            self.rule_cache.config = Arc::new(LineConfig::with_detected(&rules, &kv));
            self.rule_cache.rules = rules;
            self.rule_cache.kv = kv;
        }
        self.rule_cache.config.clone()
    }

    /// Read the view's lines again if the fields read beyond Auto-detect
    /// changed (a Detected Field turned on or off).
    pub(crate) fn reread_if_config_changed(&mut self, cx: &mut Context<Self>) {
        let before = self.rule_cache.config.clone();
        if !Arc::ptr_eq(&before, &self.line_config()) {
            self.extraction_rules_changed(cx);
        }
    }

    /// Read the rules' fields of the rows just put in the view, and detect
    /// their Filter columns.
    pub(crate) fn read_rule_fields(&mut self) {
        let config = self.line_config();
        if config.is_empty() && self.all_entries.iter().all(|e| e.extra.is_empty()) {
            return;
        }
        config.apply(&mut self.all_entries);
        self.after_rule_fields_read();
    }

    /// The fields of a new live line.
    pub(crate) fn read_rule_fields_of(&mut self, entry: &mut crate::model::LogEntry) {
        let config = self.line_config();
        if !config.is_empty() {
            entry.extra = config.extra_spans(entry);
        }
    }

    fn after_rule_fields_read(&mut self) {
        self.filter_index.rebuild_fields(&self.all_entries);
        let fitted = crate::column::FieldMaxChars::scan_entries(&self.all_entries);
        self.column_layout.max_chars.fields = fitted.fields;
    }

    /// Read the open view's lines again with the changed rules, in the
    /// background; the view stays usable meanwhile.
    pub(crate) fn extraction_rules_changed(&mut self, cx: &mut Context<Self>) {
        self.schedule_session_save(cx);
        let config = self.line_config();
        self.rule_cache.generation += 1;
        let generation = self.rule_cache.generation;
        let rows = self.all_entries.len();
        let lines = crate::extract::snapshot(&self.all_entries);
        self.rule_cache.reading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let read = cx.background_spawn(async move { config.read_snapshot(&lines) }).await;
            this.update(cx, |this, cx| {
                if this.rule_cache.generation != generation {
                    return;
                }
                this.rule_cache.reading = false;
                if this.all_entries.len() == rows {
                    for (entry, extra) in this.all_entries.iter_mut().zip(read) {
                        entry.extra = extra;
                    }
                    this.after_rule_fields_read();
                } else {
                    // Live lines came meanwhile: read them all now.
                    this.read_rule_fields();
                }
                this.apply_filters_and_refresh(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Add a rule at the end (lowest priority) and show its column.
    pub fn add_extraction_rule(
        &mut self,
        name: &str,
        pattern: &str,
        scope: RuleScope,
        description: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), crate::extract::PatternError> {
        crate::extract::compile_pattern(pattern)?;
        let Some(project) = self.active_project_mut() else { return Ok(()) };
        let rule = ExtractionRule {
            id: next_rule_id(&project.extraction_rules),
            name: name.trim().to_string(),
            pattern: pattern.to_string(),
            scope,
            description: description.to_string(),
            enabled: true,
        };
        project.columns.set_shown(&rule.name, true);
        project.extraction_rules.push(rule);
        self.extraction_rules_changed(cx);
        Ok(())
    }

    /// Change the pattern of a rule ("Edit").
    pub fn update_extraction_rule(
        &mut self,
        id: u64,
        name: &str,
        pattern: &str,
        scope: RuleScope,
        description: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), crate::extract::PatternError> {
        crate::extract::compile_pattern(pattern)?;
        let Some(project) = self.active_project_mut() else { return Ok(()) };
        let Some(rule) = project.extraction_rules.iter_mut().find(|r| r.id == id) else { return Ok(()) };
        let old = std::mem::replace(&mut rule.name, name.trim().to_string());
        rule.pattern = pattern.to_string();
        rule.scope = scope;
        rule.description = description.to_string();
        let new = rule.name.clone();
        if old != new {
            rename_shown_field(project, &old, &new);
        }
        self.extraction_rules_changed(cx);
        Ok(())
    }

    /// Turn a rule on or off. Off, its column is no longer read; a Filter
    /// condition on it is kept.
    pub fn set_extraction_rule_enabled(&mut self, id: u64, enabled: bool, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        let Some(rule) = project.extraction_rules.iter_mut().find(|r| r.id == id) else { return };
        rule.enabled = enabled;
        if enabled {
            let name = rule.name.clone();
            project.columns.set_shown(&name, true);
        }
        self.extraction_rules_changed(cx);
    }

    /// Rename the field a rule reads.
    pub fn rename_extraction_rule(&mut self, id: u64, name: &str, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        let Some(rule) = project.extraction_rules.iter_mut().find(|r| r.id == id) else { return };
        let name = name.trim();
        if name.is_empty() || rule.name == name {
            return;
        }
        let old = std::mem::replace(&mut rule.name, name.to_string());
        rename_shown_field(project, &old, name);
        self.extraction_rules_changed(cx);
    }

    pub fn remove_extraction_rule(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        let Some(at) = project.extraction_rules.iter().position(|r| r.id == id) else { return };
        let rule = project.extraction_rules.remove(at);
        if !project.extraction_rules.iter().any(|r| r.name == rule.name) {
            project.columns.set_shown(&rule.name, false);
        }
        self.rule_cache.removed.push(rule);
        self.extraction_rules_changed(cx);
    }

    /// Drag a rule in the *Rules* group: its place is its priority.
    pub fn move_extraction_rule(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        let rules = &mut project.extraction_rules;
        if from >= rules.len() || from == to {
            return;
        }
        let r = rules.remove(from);
        rules.insert(to.min(rules.len()), r);
        self.extraction_rules_changed(cx);
    }
}

/// Keep a renamed field's place among the Field Columns shown.
fn rename_shown_field(project: &mut crate::workspace::project::Project, old: &str, new: &str) {
    let still_read = project.extraction_rules.iter().any(|r| r.name == old);
    let fields = &mut project.columns.fields;
    match fields.iter().position(|f| f == old) {
        Some(i) if !still_read && !fields.iter().any(|f| f == new) => fields[i] = new.to_string(),
        _ => project.columns.set_shown(new, true),
    }
}
