//! Logic: Log Formats. The library lives beside the Servers and is shared by
//! every Project; which format a Log Space or Source File uses belongs to the
//! Project. Each cached file remembers the format it was read with; a file
//! whose format changed is read again from its stored lines (in the
//! background when the user changes it), keeping line numbers and Markers.

use super::AppView;
use crate::log_format::{CompiledFormat, FormatChoice, LogFormat, LogFormatStore, compile_template, resolve};
use crate::workspace::log_space::LogSpace;
use gpui::{AppContext as _, Context};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// The library and its compiled templates.
pub struct FormatLibrary {
    pub store: LogFormatStore,
    path: PathBuf,
    compiled: HashMap<String, (String, Arc<CompiledFormat>)>,
    /// Bumped by every change of a choice; a background read of an older
    /// one is dropped.
    generation: u64,
    /// Whether files are being read again with a changed format.
    pub reading: bool,
    /// The fields of the formats the open view uses, as Field Columns.
    pub view_fields: Vec<String>,
}

impl FormatLibrary {
    pub fn load(path: PathBuf) -> Self {
        Self { store: LogFormatStore::load(&path), path, compiled: HashMap::new(), generation: 0, reading: false, view_fields: Vec::new() }
    }

    fn save(&self) {
        if let Err(e) = self.store.save_to(&self.path) {
            eprintln!("could not save Log Formats: {e}");
        }
    }

    /// The compiled template of the format `id`, if it compiles.
    pub fn compiled(&mut self, id: &str) -> Option<Arc<CompiledFormat>> {
        let format = self.store.get(id)?;
        if let Some((template, c)) = self.compiled.get(id) {
            if *template == format.template {
                return Some(c.clone());
            }
        }
        let c = Arc::new(compile_template(&format.template).ok()?);
        self.compiled.insert(id.to_string(), (format.template.clone(), c.clone()));
        Some(c)
    }
}

/// Where a Log Format is chosen, for the question before deleting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatUse {
    pub project: String,
    /// The Log Space, or "space / file".
    pub place: String,
}

impl AppView {
    pub fn log_formats(&self) -> &[LogFormat] {
        &self.formats.store.formats
    }

    /// The format a file of `space` is read with, by the resolution order.
    fn wanted_format(&self, space: &LogSpace, file_id: &str, file_name: &str) -> Option<String> {
        let choices = self.active_project().map(|p| p.format_choices.clone()).unwrap_or_default();
        resolve(self.log_formats(), &choices, &space.id, file_id, file_name).map(|f| f.id.clone())
    }

    /// The files of `space` and the format each should be read with.
    fn wanted_formats(&self, space: &LogSpace) -> Vec<(String, Option<String>)> {
        space
            .files
            .iter()
            .map(|f| (f.file_id.clone(), self.wanted_format(space, &f.file_id, &f.display_name)))
            .collect()
    }

    /// Read again, now, the cached files of `space` read with another format
    /// than theirs (a file loaded for the first time, a glob that matches).
    pub(crate) fn apply_log_formats(&mut self, space: &LogSpace) {
        for (file_id, wanted) in self.wanted_formats(space) {
            let have = self.parsed_files.get(&file_id).map(|p| p.format_id.clone());
            if have.is_none() || have == Some(wanted.clone()) {
                continue;
            }
            let compiled = wanted.as_deref().and_then(|id| self.formats.compiled(id));
            if let Some(parsed) = self.parsed_files.get_mut(&file_id) {
                parsed.reformat(wanted, compiled);
            }
        }
    }

    /// Read the open Log Space's files whose format changed again, in the
    /// background, then rebuild the view (time order, Filter columns).
    pub(crate) fn reread_formats(&mut self, cx: &mut Context<Self>) {
        let Some(space) = self.active_space().cloned() else { return };
        let mut jobs = Vec::new();
        for (file_id, wanted) in self.wanted_formats(&space) {
            let Some(parsed) = self.parsed_files.get(&file_id) else { continue };
            if parsed.format_id == wanted {
                continue;
            }
            let compiled = wanted.as_deref().and_then(|id| self.formats.compiled(id));
            jobs.push((file_id, wanted, compiled, crate::parser::text::stored_lines(&parsed.entries)));
        }
        self.formats.generation += 1;
        if jobs.is_empty() {
            self.formats.reading = false;
            self.rebuild_open_view_keeping_position(cx);
            return;
        }
        let generation = self.formats.generation;
        self.formats.reading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let read = cx
                .background_spawn(async move {
                    jobs.into_iter()
                        .map(|(id, wanted, compiled, lines)| {
                            let fresh = crate::parser::text::reread(&lines, compiled.as_deref());
                            (id, wanted, compiled, lines, fresh)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                if this.formats.generation != generation {
                    return;
                }
                this.formats.reading = false;
                for (id, wanted, compiled, lines, fresh) in read {
                    let Some(parsed) = this.parsed_files.get_mut(&id) else { continue };
                    let same = parsed.entries.len() == lines.len()
                        && parsed.entries.iter().zip(&lines).all(|(e, l)| e.index == l.0 && e.generation == l.1);
                    if same {
                        for (entry, f) in parsed.entries.iter_mut().zip(fresh) {
                            crate::parser::text::adopt(entry, f);
                        }
                        parsed.adopt_format(wanted, compiled);
                    } else {
                        // Live lines came meanwhile: read them all now.
                        parsed.reformat(wanted, compiled);
                    }
                }
                this.rebuild_open_view_keeping_position(cx);
            })
            .ok();
        })
        .detach();
    }

    /// The format chosen by hand for a Log Space (`None`: not chosen).
    pub fn space_format_choice(&self, space_id: &str) -> Option<FormatChoice> {
        self.active_project()?.format_choices.spaces.get(space_id).cloned()
    }

    pub fn file_format_choice(&self, file_id: &str) -> Option<FormatChoice> {
        self.active_project()?.format_choices.files.get(file_id).cloned()
    }

    /// Choose the Log Format of a Log Space (`None`: leave it to the globs).
    pub fn set_space_format(&mut self, space_id: &str, choice: Option<FormatChoice>, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        match choice {
            Some(c) => project.format_choices.spaces.insert(space_id.to_string(), c),
            None => project.format_choices.spaces.remove(space_id),
        };
        self.schedule_session_save(cx);
        self.reread_formats(cx);
    }

    /// Choose the Log Format of one Source File (`None`: as its Log Space).
    pub fn set_file_format(&mut self, file_id: &str, choice: Option<FormatChoice>, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        match choice {
            Some(c) => project.format_choices.files.insert(file_id.to_string(), c),
            None => project.format_choices.files.remove(file_id),
        };
        self.schedule_session_save(cx);
        self.reread_formats(cx);
    }

    /// The Log Formats the open view's files are read with, in file order.
    pub fn formats_in_view(&mut self) -> Vec<(String, Arc<CompiledFormat>)> {
        let Some(space) = self.active_space().cloned() else { return Vec::new() };
        let mut out: Vec<(String, Arc<CompiledFormat>)> = Vec::new();
        for f in &space.files {
            let Some(id) = self.parsed_files.get(&f.file_id).and_then(|p| p.format_id.clone()) else { continue };
            if out.iter().any(|(i, _)| *i == id) {
                continue;
            }
            if let Some(c) = self.formats.compiled(&id) {
                out.push((id, c));
            }
        }
        out
    }

    /// The fields of the formats in use that become Field Columns, in
    /// template order.
    pub fn format_fields_in_view(&mut self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, c) in self.formats_in_view() {
            for f in c.column_fields() {
                if !out.iter().any(|o| o == f) {
                    out.push(f.to_string());
                }
            }
        }
        out
    }

    // ── The library ─────────────────────────────────────────────────────────

    /// Save a format (a new one when `id` is `None`); returns its id. Files
    /// read with it are read again.
    pub fn save_log_format(
        &mut self,
        id: Option<&str>,
        name: &str,
        template: &str,
        globs: Vec<String>,
        cx: &mut Context<Self>,
    ) -> String {
        let id = match id {
            Some(id) => {
                self.formats.store.update(id, name, template, globs);
                id.to_string()
            }
            None => self.formats.store.add(name, template, globs),
        };
        self.formats.save();
        self.forget_format_reads(&id);
        self.reread_formats(cx);
        id
    }

    /// Files read with format `id` must be read again: its template changed.
    fn forget_format_reads(&mut self, id: &str) {
        for parsed in self.parsed_files.values_mut() {
            if parsed.format_id.as_deref() == Some(id) {
                parsed.format_id = Some(String::new());
            }
        }
    }

    pub fn duplicate_log_format(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.formats.store.duplicate(id).is_some() {
            self.formats.save();
            cx.notify();
        }
    }

    /// Where the format `id` is chosen, in every Project.
    pub fn log_format_uses(&self, id: &str) -> Vec<FormatUse> {
        let chosen = FormatChoice::Format(id.to_string());
        let mut uses = Vec::new();
        for p in &self.workspace_state.projects {
            for space in &p.log_spaces {
                if p.format_choices.spaces.get(&space.id) == Some(&chosen) {
                    uses.push(FormatUse { project: p.name.clone(), place: space.name.clone() });
                }
                for f in &space.files {
                    if p.format_choices.files.get(&f.file_id) == Some(&chosen) {
                        uses.push(FormatUse { project: p.name.clone(), place: format!("{} / {}", space.name, f.display_name) });
                    }
                }
            }
        }
        uses
    }

    /// Delete a format, asking first when a Log Space or file uses it.
    pub fn request_delete_log_format(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.log_format_uses(id).is_empty() {
            self.delete_log_format(id, cx);
        } else {
            self.confirm = Some(super::ConfirmAction::DeleteLogFormat { format_id: id.to_string() });
            cx.notify();
        }
    }

    /// Delete a format; the places that chose it go back to Auto-detect.
    pub fn delete_log_format(&mut self, id: &str, cx: &mut Context<Self>) {
        self.formats.store.remove(id);
        self.formats.save();
        for p in &mut self.workspace_state.projects {
            p.format_choices.forget(id);
        }
        self.schedule_session_save(cx);
        self.reread_formats(cx);
    }
}
