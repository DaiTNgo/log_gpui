//! Logic: adding a Remote Command to a Remote Log Space, from the "Command"
//! tab of the picker: choose a Server, type the command or fill a Command
//! Preset, see exactly what will run and as whom, then Add. Also what the
//! menu of a Remote Command does (Restart, Restart on exit).

use super::AppView;
use super::remote_picker::{PickerTab, PickerTarget};
use crate::live::SourceCommand;
use crate::remote::command::{DEFAULT_PRESET_LINES, unedited_preset};
use crate::remote::{PresetFields, ServerStore, check_elevation, shell_quote};
use crate::workspace::log_space::{AddCommandError, RemoteTarget};
use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

/// A Command Preset the form can fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetKind {
    Journal,
    Docker,
    Kube,
}

impl PresetKind {
    pub fn label(self) -> &'static str {
        match self {
            PresetKind::Journal => "journalctl",
            PresetKind::Docker => "Docker",
            PresetKind::Kube => "Kubernetes",
        }
    }

    /// The name of what the preset reads the log of.
    pub fn subject(self) -> &'static str {
        match self {
            PresetKind::Journal => "Unit",
            PresetKind::Docker => "Container",
            PresetKind::Kube => "Pod",
        }
    }
}

/// The fields of the form, created while rendering where a window exists.
pub struct CommandInputs {
    /// The command that will run, always editable.
    pub command: Entity<InputState>,
    /// Unit, container or pod.
    pub subject: Entity<InputState>,
    pub namespace: Entity<InputState>,
    pub container: Entity<InputState>,
    pub context: Entity<InputState>,
    pub lines: Entity<InputState>,
}

#[derive(Default)]
pub struct CommandForm {
    /// `None`: the command is typed.
    pub preset: Option<PresetKind>,
    pub inputs: Option<CommandInputs>,
    /// A command to put in the command field at the next render.
    pub set_command: Option<String>,
    /// What the preset's fields last wrote (to tell a change of the fields
    /// from typing in the command field).
    pub last_rendered: Option<String>,
    /// Values found on the Server for the subject field.
    pub suggestions: Vec<String>,
    pub suggestions_error: Option<String>,
    pub suggestions_loading: bool,
    suggestion_request: u64,
    /// Why the command cannot be added.
    pub error: Option<String>,
}

/// What the listing of units, containers or pods printed.
pub fn parse_listing(kind: PresetKind, stdout: &str) -> Vec<String> {
    let mut names: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let first = line.split_whitespace().next()?;
            let name = match kind {
                PresetKind::Journal => first.strip_suffix(".service").unwrap_or(first).trim_start_matches('●'),
                PresetKind::Docker => first,
                PresetKind::Kube => first.strip_prefix("pod/").unwrap_or(first),
            };
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The fixed, read-only command that lists what a preset can read the log of.
pub fn listing_command(kind: PresetKind, namespace: &str, context: &str) -> String {
    match kind {
        PresetKind::Journal => "systemctl list-units --type=service --all --no-legend --plain".into(),
        PresetKind::Docker => "docker ps --format '{{.Names}}'".into(),
        PresetKind::Kube => {
            let mut command = String::from("kubectl get pods");
            if !context.is_empty() {
                command.push_str(&format!(" --context {}", shell_quote(context)));
            }
            if !namespace.is_empty() {
                command.push_str(&format!(" -n {}", shell_quote(namespace)));
            }
            command.push_str(" -o name");
            command
        }
    }
}

impl AppView {
    /// Open the picker on the Command tab.
    pub fn open_remote_command_picker(&mut self, target: PickerTarget, cx: &mut Context<Self>) {
        self.open_remote_picker(target, cx);
        if let Some(picker) = self.remote.picker.as_mut() {
            picker.tab = PickerTab::Command;
            picker.command = CommandForm::default();
        }
        cx.notify();
    }

    pub fn add_remote_command_dialog(&mut self, space_id: &str, cx: &mut Context<Self>) {
        self.open_remote_command_picker(PickerTarget::AddTo(space_id.to_string()), cx);
    }

    pub fn picker_set_tab(&mut self, tab: PickerTab, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if picker.tab == tab {
            return;
        }
        picker.tab = tab;
        let load_files = tab == PickerTab::Files && picker.entries.is_empty() && picker.server_id.is_some();
        if load_files {
            self.picker_load(None, cx);
        }
        cx.notify();
    }

    /// Create the fields of the command form (once, where a window exists).
    pub fn sync_command_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if picker.tab != PickerTab::Command {
            return;
        }
        if picker.command.inputs.is_none() {
            let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                cx.subscribe_in(&input, window, |this: &mut Self, _, event: &InputEvent, _window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.command_input_changed(cx);
                    }
                })
                .detach();
                input
            };
            let inputs = CommandInputs {
                command: input("journalctl -f -u nginx", window, cx),
                subject: input("", window, cx),
                namespace: input("namespace (optional)", window, cx),
                container: input("container (optional)", window, cx),
                context: input("context (optional)", window, cx),
                lines: input("1000", window, cx),
            };
            inputs.lines.update(cx, |state, cx| state.set_value(DEFAULT_PRESET_LINES.to_string(), window, cx));
            if let Some(picker) = self.remote.picker.as_mut() {
                picker.command.inputs = Some(inputs);
            }
        }
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if let (Some(text), Some(inputs)) = (picker.command.set_command.take(), picker.command.inputs.as_ref()) {
            let command = inputs.command.clone();
            command.update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    /// The fields typed in the form for the chosen preset.
    pub fn command_preset_fields(&self, cx: &gpui::App) -> Option<PresetFields> {
        let form = &self.remote.picker.as_ref()?.command;
        let kind = form.preset?;
        let inputs = form.inputs.as_ref()?;
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let lines = text(&inputs.lines).parse().unwrap_or(DEFAULT_PRESET_LINES);
        Some(match kind {
            PresetKind::Journal => PresetFields::Journal { unit: text(&inputs.subject), lines },
            PresetKind::Docker => PresetFields::Docker { container: text(&inputs.subject), lines },
            PresetKind::Kube => PresetFields::Kube {
                pod: text(&inputs.subject),
                namespace: text(&inputs.namespace),
                container: text(&inputs.container),
                context: text(&inputs.context),
                lines,
            },
        })
    }

    /// A field of the form changed: the command follows the preset's fields.
    fn command_input_changed(&mut self, cx: &mut Context<Self>) {
        // The command field itself also reports changes (typing, or the text
        // set below): only the preset's fields write the command.
        let Some(fields) = self.command_preset_fields(cx) else { return };
        let rendered = fields.command();
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(inputs) = picker.command.inputs.as_ref() else { return };
        let current = inputs.command.read(cx).value().to_string();
        let fields_changed = picker.command.last_rendered.as_deref() != Some(rendered.as_str());
        picker.command.last_rendered = Some(rendered.clone());
        if fields_changed && current != rendered {
            picker.command.set_command = Some(rendered);
        }
        picker.command.error = None;
        cx.notify();
    }

    /// Choose a Command Preset (or none: type the command).
    pub fn command_select_preset(&mut self, preset: Option<PresetKind>, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        picker.command.preset = preset;
        picker.command.suggestions.clear();
        picker.command.suggestions_error = None;
        picker.command.last_rendered = None;
        picker.command.error = None;
        if preset.is_some() {
            self.command_input_changed(cx);
            self.command_load_suggestions(cx);
        }
        cx.notify();
    }

    /// Fill the subject field with a value found on the Server.
    pub fn command_pick_suggestion(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(inputs) = self.remote.picker.as_ref().and_then(|p| p.command.inputs.as_ref()) else { return };
        let subject = inputs.subject.clone();
        let value = value.to_string();
        subject.update(cx, |state, cx| state.set_value(value, window, cx));
    }

    /// Run the fixed listing of the preset on the chosen Server (once per
    /// opening of the form, and again on Refresh).
    pub fn command_load_suggestions(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let (Some(kind), Some(server_id)) = (picker.command.preset, picker.server_id.clone()) else { return };
        let (namespace, context) = match picker.command.inputs.as_ref() {
            Some(i) => (i.namespace.read(cx).value().trim().to_string(), i.context.read(cx).value().trim().to_string()),
            None => (String::new(), String::new()),
        };
        picker.command.suggestions_loading = true;
        picker.command.suggestions_error = None;
        picker.command.suggestion_request += 1;
        let request = picker.command.suggestion_request;
        let command = listing_command(kind, &namespace, &context);
        let hub = self.remote_hub(cx);
        let store: ServerStore = self.remote.servers.clone();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                let session = hub.session(&store, &server_id).await.map_err(|e| e.message())?;
                let out = session.exec(&command).await?;
                if out.exit_status == Some(0) {
                    Ok(parse_listing(kind, &String::from_utf8_lossy(&out.stdout)))
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let first = stderr.lines().map(str::trim).find(|l| !l.is_empty());
                    Err(match (first, out.exit_status) {
                        (Some(line), _) => line.to_string(),
                        (None, Some(code)) => format!("{command} exited with code {code}"),
                        (None, None) => format!("{command} did not finish"),
                    })
                }
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let Some(picker) = this.remote.picker.as_mut() else { return };
                if picker.command.suggestion_request != request {
                    return;
                }
                picker.command.suggestions_loading = false;
                match result {
                    Ok(Ok(names)) => picker.command.suggestions = names,
                    Ok(Err(message)) => picker.command.suggestions_error = Some(message),
                    Err(e) => picker.command.suggestions_error = Some(format!("Could not reach the server: {e}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// The command as typed now.
    pub fn command_text(&self, cx: &gpui::App) -> String {
        self.remote
            .picker
            .as_ref()
            .and_then(|p| p.command.inputs.as_ref())
            .map(|i| i.command.read(cx).value().trim().to_string())
            .unwrap_or_default()
    }

    /// `(server name, user)` of the chosen Server.
    pub fn command_server(&self) -> Option<(String, String, String)> {
        let id = self.remote.picker.as_ref()?.server_id.clone()?;
        let server = self.remote.servers.get(&id)?;
        Some((id, server.name.clone(), server.user.clone()))
    }

    /// Why the command in the form cannot be added, if it cannot.
    pub fn command_form_error(&self, cx: &gpui::App) -> Option<String> {
        let (_, _, user) = self.command_server()?;
        let command = self.command_text(cx);
        if command.is_empty() {
            return None;
        }
        check_elevation(&command)
            .err()
            .map(|_| format!("Commands run as {user}; elevation is not allowed"))
    }

    /// Whether the preset's command was edited by hand.
    pub fn command_is_edited(&self, cx: &gpui::App) -> bool {
        match self.command_preset_fields(cx) {
            Some(fields) => unedited_preset(&self.command_text(cx), Some(&fields)).is_none(),
            None => false,
        }
    }

    /// Add: the command goes to the New Log Space or the Remote Log Space.
    pub fn command_add(&mut self, cx: &mut Context<Self>) {
        let Some((server_id, server_name, _)) = self.command_server() else { return };
        let command = self.command_text(cx);
        if command.is_empty() || self.command_form_error(cx).is_some() {
            return;
        }
        let preset = self.command_preset_fields(cx);
        let Some(picker) = self.remote.picker.take() else { return };
        let entry = (server_id, server_name, command, preset);
        match picker.target {
            PickerTarget::NewSpace => {
                if !self.remote.new_space_commands.iter().any(|c| c.0 == entry.0 && c.2 == entry.2) {
                    self.remote.new_space_commands.push(entry);
                }
            }
            PickerTarget::AddTo(space_id) => self.add_remote_commands_to_space(&space_id, vec![entry], cx),
        }
        cx.notify();
    }

    /// Add Remote Commands to a Remote Log Space; one it already has is
    /// skipped with a message.
    pub fn add_remote_commands_to_space(
        &mut self,
        space_id: &str,
        commands: Vec<(String, String, String, Option<PresetFields>)>,
        cx: &mut Context<Self>,
    ) {
        self.persist_active_session();
        let outcome = self
            .active_project_mut()
            .and_then(|p| p.log_space_mut(space_id))
            .map(|s| s.add_remote_commands(&commands));
        match outcome {
            Some(Ok(outcome)) if !outcome.skipped.is_empty() => {
                self.notice = Some(format!("Already in this Log Space: {}", outcome.skipped.join(", ")));
            }
            Some(Err(AddCommandError::Elevation(e))) => {
                self.notice = Some(format!("Elevation is not allowed: {}", e.word));
            }
            _ => {}
        }
        self.refresh_after_space_change(cx);
    }

    /// Drop a command chosen for the New Log Space.
    pub fn new_space_remove_command(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.remote.new_space_commands.len() {
            self.remote.new_space_commands.remove(index);
        }
        cx.notify();
    }

    // ── The menu of a Remote Command ───────────────────────────────────────

    /// Run a Remote Command again from its start.
    pub fn restart_remote_command(&mut self, file_id: &str, cx: &mut Context<Self>) {
        if let Some(source) = self.live.sources.get(file_id) {
            source.handle.send(SourceCommand::Restart);
        }
        cx.notify();
    }

    /// Turn Restart on exit on or off for a Remote Command; it is kept with
    /// the Log Space.
    pub fn set_restart_on_exit(&mut self, space_id: &str, file_id: &str, on: bool, cx: &mut Context<Self>) {
        if let Some(file) = self.active_project_mut().and_then(|p| p.log_space_mut(space_id)).and_then(|s| s.file_mut(file_id)) {
            if let Some(RemoteTarget::Command { restart_on_exit, .. }) = file.remote.as_mut().map(|r| &mut r.target) {
                *restart_on_exit = on;
            }
        }
        if let Some(source) = self.live.sources.get(file_id) {
            source.handle.send(SourceCommand::SetRestartOnExit(on));
        }
        self.persist_active_session();
        cx.notify();
    }
}
