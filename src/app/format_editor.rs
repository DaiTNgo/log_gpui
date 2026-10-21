//! Logic: the Log Format editor. The template is compiled at every keystroke
//! and previewed on real lines of the open view, each field in its own color;
//! while the template is not usable the preview keeps the last usable one and
//! Save is off, and the error shows only once the user stops typing (rule 40).

use super::AppView;
use crate::log_format::{BUILT_IN, FormatPreview, PREVIEW_LINES, TemplateError, compile_template, preview_format};
use crate::pattern_field::{PatternField, SETTLE};
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

pub struct FormatEditor {
    /// The format being edited, or `None` for a new one.
    pub id: Option<String>,
    /// The Log Space to read with the format once saved (opened from its
    /// picker with "New Log Format…").
    pub apply_to_space: Option<String>,
    /// The Source File to read with the format once saved (opened from its
    /// "Log Format…" with no format applying).
    pub apply_to_file: Option<String>,
    pub name_input: Entity<InputState>,
    pub template_input: Entity<InputState>,
    /// File name globs, separated by commas.
    pub globs_input: Entity<InputState>,
    pub template: PatternField<TemplateError>,
    pub name: PatternField<String>,
    /// The preview of the last usable template.
    pub preview: Option<FormatPreview>,
    /// The template typed is not usable: the preview is of an earlier one.
    pub stale: bool,
    _subscriptions: Vec<Subscription>,
}

impl FormatEditor {
    pub fn can_save(&self) -> bool {
        self.template.is_valid() && self.name.is_valid()
    }
}

impl AppView {
    /// Open the editor on the format `id`, or on a new one.
    pub fn open_format_editor(
        &mut self,
        id: Option<&str>,
        apply_to_space: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = id.and_then(|id| self.formats.store.get(id)).cloned();
        let (name, template, globs) = match &existing {
            Some(f) => (f.name.clone(), f.template.clone(), f.globs.join(", ")),
            None => (String::new(), String::new(), String::new()),
        };
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Format name").default_value(name));
        let template_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("%{TIMESTAMP:timestamp} [%{LEVEL:level}] %{GREEDY:message}")
                .default_value(template)
        });
        let globs_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("File names it reads on its own, e.g. access*.log").default_value(globs));
        let subscriptions = vec![
            cx.subscribe(&name_input, |this: &mut Self, _, event: &InputEvent, cx| match event {
                InputEvent::Change => this.format_name_changed(cx),
                InputEvent::Blur | InputEvent::PressEnter { .. } => {
                    if let Some(e) = this.format_editor.as_mut() {
                        e.name.leave();
                        cx.notify();
                    }
                }
                _ => {}
            }),
            cx.subscribe(&template_input, |this: &mut Self, _, event: &InputEvent, cx| match event {
                InputEvent::Change => this.format_template_changed(cx),
                InputEvent::Blur | InputEvent::PressEnter { .. } => {
                    if let Some(e) = this.format_editor.as_mut() {
                        e.template.leave();
                        cx.notify();
                    }
                }
                _ => {}
            }),
        ];
        let focus = if existing.is_some() { template_input.clone() } else { name_input.clone() };
        focus.update(cx, |s, cx| s.focus(window, cx));
        self.format_editor = Some(FormatEditor {
            id: existing.map(|f| f.id),
            apply_to_space,
            apply_to_file: None,
            name_input,
            template_input,
            globs_input,
            template: PatternField::default(),
            name: PatternField::default(),
            preview: None,
            stale: false,
            _subscriptions: subscriptions,
        });
        self.format_name_changed(cx);
        self.format_template_changed(cx);
    }

    /// "Log Format…" of a Log Space (`file_id` `None`) or of a Source File:
    /// the editor opens on the Log Format that applies; with none, on a new
    /// one whose template is a sample line of the file, read by the format
    /// once saved (by the file, or by the Log Space).
    pub fn edit_log_format(&mut self, space_id: &str, file_id: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.applied_format_id(space_id, file_id) {
            self.open_format_editor(Some(&id), None, window, cx);
            return;
        }
        let sample = self.sample_line(space_id, file_id);
        let apply_to_space = file_id.is_none().then(|| space_id.to_string());
        self.open_format_editor(None, apply_to_space, window, cx);
        if let Some(editor) = self.format_editor.as_mut() {
            editor.apply_to_file = file_id.map(str::to_string);
        }
        if let Some(line) = sample {
            if let Some(input) = self.format_editor.as_ref().map(|e| e.template_input.clone()) {
                input.update(cx, |s, cx| s.set_value(line, window, cx));
            }
            self.format_template_changed(cx);
        }
    }

    /// The Log Format a Source File is read with (a Log Space: the one chosen
    /// for it, else the one of its first file that has one).
    fn applied_format_id(&self, space_id: &str, file_id: Option<&str>) -> Option<String> {
        use crate::log_format::{FormatChoice, resolve};
        let choices = self.active_project()?.format_choices.clone();
        let space = self.active_project()?.log_spaces.iter().find(|s| s.id == space_id)?;
        let of_file = |file_id: &str, name: &str| {
            resolve(self.log_formats(), &choices, space_id, file_id, name).map(|f| f.id.clone())
        };
        match file_id {
            Some(file_id) => of_file(file_id, &space.file(file_id)?.display_name),
            None => match choices.spaces.get(space_id) {
                Some(FormatChoice::Format(id)) if self.formats.store.get(id).is_some() => Some(id.clone()),
                Some(_) => None,
                None => space.files.iter().find_map(|f| of_file(&f.file_id, &f.display_name)),
            },
        }
    }

    /// The first line of the file that is not a Continuation Line (of a
    /// Log Space: of its first file that has one).
    fn sample_line(&self, space_id: &str, file_id: Option<&str>) -> Option<String> {
        let space = self.active_project()?.log_spaces.iter().find(|s| s.id == space_id)?;
        let first = |id: &str| {
            self.parsed_files.get(id)?.entries.iter().find(|e| !e.is_continuation()).map(|e| e.raw_line.to_string())
        };
        match file_id {
            Some(id) => first(id),
            None => space.files.iter().find_map(|f| first(&f.file_id)),
        }
    }

    pub fn close_format_editor(&mut self, cx: &mut Context<Self>) {
        if self.format_editor.take().is_some() {
            cx.notify();
        }
    }

    fn format_name_changed(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.format_editor.as_ref() else { return };
        let name = editor.name_input.read(cx).value().trim().to_string();
        let own = editor.id.clone();
        let taken = self.log_formats().iter().any(|f| Some(&f.id) != own.as_ref() && f.name.eq_ignore_ascii_case(&name));
        let result = if name.is_empty() {
            Err("Give the format a name".to_string())
        } else if taken {
            Err(format!("\"{name}\" is already a Log Format — pick another name"))
        } else {
            Ok(())
        };
        let Some(editor) = self.format_editor.as_mut() else { return };
        let edit = editor.name.edit(result);
        self.settle_format_field(edit, false, cx);
        cx.notify();
    }

    /// The lines the preview shows: up to 20 lines that are not Continuation
    /// Lines, from the selected row on (else from the top).
    fn format_preview_rows(&self) -> Vec<usize> {
        let start = self.selected_row_pos.and_then(|p| self.visible_indices.get(p).copied()).unwrap_or(0);
        let parents = |range: std::ops::Range<usize>| {
            range.filter(|&i| !self.all_entries[i].is_continuation()).collect::<Vec<_>>()
        };
        let mut rows: Vec<usize> = parents(start..self.all_entries.len()).into_iter().take(PREVIEW_LINES).collect();
        if rows.len() < PREVIEW_LINES {
            let before = parents(0..start);
            let need = PREVIEW_LINES - rows.len();
            rows.splice(0..0, before[before.len().saturating_sub(need)..].iter().copied());
        }
        rows
    }

    fn format_template_changed(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.format_editor.as_ref() else { return };
        let text = editor.template_input.read(cx).value().to_string();
        let compiled = compile_template(&text);
        let preview = compiled.as_ref().ok().map(|format| {
            let rows = self.format_preview_rows();
            let shown: Vec<(usize, &str)> = rows.iter().map(|&i| (i, self.all_entries[i].raw_line.as_str())).collect();
            let all = self
                .all_entries
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.is_continuation())
                .map(|(i, e)| (i, e.raw_line.as_str()));
            preview_format(format, &shown, all)
        });
        let Some(editor) = self.format_editor.as_mut() else { return };
        let edit = editor.template.edit(compiled.map(|_| ()));
        match preview {
            Some(p) => {
                editor.preview = Some(p);
                editor.stale = false;
            }
            None => editor.stale = editor.preview.is_some(),
        }
        self.settle_format_field(edit, true, cx);
        cx.notify();
    }

    fn settle_format_field(&mut self, edit: u64, template: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            this.update(cx, |this, cx| {
                let Some(e) = this.format_editor.as_mut() else { return };
                let shown = if template { e.template.settle(edit) } else { e.name.settle(edit) };
                if shown {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Copy a built-in template into the editor (it stays the user's own).
    pub fn use_built_in_format(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((name, template, globs)) = BUILT_IN.get(ix).copied() else { return };
        let Some(editor) = self.format_editor.as_ref() else { return };
        let (name_input, template_input, globs_input) =
            (editor.name_input.clone(), editor.template_input.clone(), editor.globs_input.clone());
        if name_input.read(cx).value().trim().is_empty() {
            name_input.update(cx, |s, cx| s.set_value(name.to_string(), window, cx));
            self.format_name_changed(cx);
        }
        if globs_input.read(cx).value().trim().is_empty() {
            globs_input.update(cx, |s, cx| s.set_value(globs.to_string(), window, cx));
        }
        template_input.update(cx, |s, cx| s.set_value(template.to_string(), window, cx));
        self.format_template_changed(cx);
    }

    /// The view goes to the first line the template does not match.
    pub fn jump_to_first_unmatched(&mut self, cx: &mut Context<Self>) {
        let target = self.format_editor.as_ref().and_then(|e| e.preview.as_ref()).and_then(|p| p.first_unmatched);
        if let Some(entry) = target {
            self.jump_to_preview_line(entry, cx);
        }
    }

    /// Save: only with a usable name and template.
    pub fn save_format_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.format_editor.as_mut() else { return };
        if !editor.can_save() {
            editor.name.leave();
            editor.template.leave();
            cx.notify();
            return;
        }
        let name = editor.name_input.read(cx).value().trim().to_string();
        let template = editor.template_input.read(cx).value().to_string();
        let globs: Vec<String> = editor
            .globs_input
            .read(cx)
            .value()
            .split(',')
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty())
            .collect();
        let (id, space, file) = (editor.id.clone(), editor.apply_to_space.clone(), editor.apply_to_file.clone());
        self.format_editor = None;
        let id = self.save_log_format(id.as_deref(), &name, &template, globs, cx);
        if let Some(space) = space {
            self.set_space_format(&space, Some(crate::log_format::FormatChoice::Format(id.clone())), cx);
        }
        if let Some(file) = file {
            self.set_file_format(&file, Some(crate::log_format::FormatChoice::Format(id)), cx);
        }
        cx.notify();
    }
}
