//! Logic: changing the Keymap from its Settings page, and saving it.

use super::AppView;
use crate::keymap::{AssignError, Command, Shortcut};
use gpui::{Context, Keystroke, Window};

/// How a Shortcut reads in a message: `⌘⇧F` on macOS, `Ctrl+Shift+F` elsewhere.
pub fn shortcut_label(shortcut: &Shortcut) -> String {
    let mac = cfg!(target_os = "macos");
    let mut out = String::new();
    if shortcut.cmd {
        out.push_str(if mac { "⌘" } else { "Ctrl+" });
    }
    if shortcut.alt {
        out.push_str(if mac { "⌥" } else { "Alt+" });
    }
    if shortcut.shift {
        out.push_str(if mac { "⇧" } else { "Shift+" });
    }
    let key = match shortcut.key.as_str() {
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "enter" => "Enter".to_string(),
        "space" => "Space".to_string(),
        "escape" => "Esc".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        key => key.to_uppercase(),
    };
    out.push_str(&key);
    out
}

impl AppView {
    /// Wait for the next key press in the Keymap settings, for `command`.
    pub fn start_recording_shortcut(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        self.keymap_recording = Some(command);
        self.keymap_message = None;
        window.focus(&self.keymap_record_focus, cx);
        cx.notify();
    }

    pub fn cancel_recording_shortcut(&mut self, cx: &mut Context<Self>) {
        self.keymap_recording = None;
        cx.notify();
    }

    /// A key pressed while recording: Escape alone cancels, anything else is
    /// added to the Command being recorded for.
    pub fn record_shortcut(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) {
        let Some(command) = self.keymap_recording else { return };
        let shortcut = Shortcut::from_keystroke(keystroke);
        if shortcut.is_modifier_only() {
            return;
        }
        if shortcut.key == "escape" && !shortcut.cmd && !shortcut.alt && !shortcut.shift {
            self.cancel_recording_shortcut(cx);
            return;
        }
        let label = shortcut_label(&shortcut);
        self.keymap_message = match self.keymap.add(command, shortcut) {
            Ok(Some(previous)) => Some(format!("{label} now runs “{}”; “{}” no longer has it.", command.name(), previous.name())),
            Ok(None) => None,
            Err(AssignError::Fixed) => Some(format!("{label} cannot be changed; choose another shortcut.")),
            Err(AssignError::ModifierOnly) => None,
        };
        self.keymap_recording = None;
        self.save_keymap(cx);
    }

    pub fn remove_shortcut(&mut self, command: Command, shortcut: &Shortcut, cx: &mut Context<Self>) {
        self.keymap.remove(command, shortcut);
        self.keymap_message = None;
        self.save_keymap(cx);
    }

    pub fn reset_shortcuts(&mut self, command: Command, cx: &mut Context<Self>) {
        self.keymap.reset(command);
        self.keymap_message = None;
        self.save_keymap(cx);
    }

    pub fn reset_all_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.keymap.reset_all();
        self.keymap_message = None;
        self.save_keymap(cx);
    }

    fn save_keymap(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.keymap.save_to(&self.keymap_path) {
            self.notice = Some(format!("Shortcuts could not be saved: {e}"));
        }
        cx.notify();
    }
}
