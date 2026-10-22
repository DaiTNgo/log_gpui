//! Keymap: which Shortcut runs which Command. Every Command has default
//! Shortcuts; the user's changes are kept in `keymap.json` beside the session,
//! as the Shortcuts of each changed Command only, so new defaults still reach
//! the Commands nobody changed.
//!
//! A Shortcut is written `cmd-shift-f`: `cmd` is ⌘ on macOS and Ctrl
//! elsewhere (the app reads Ctrl as ⌘ too). Escape, the arrow keys, `cmd-c`,
//! `cmd-1`…`cmd-9` and `cmd-tab` are Fixed Keys and cannot be taken.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Something the user can run from the Command Palette or a Shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    ToggleSearch,
    NextMatch,
    PrevMatch,
    ToggleFilter,
    ToggleFocusPane,
    MoveFocusToWindow,
    ToggleFindFilter,
    ToggleSearchHighlight,
    ShowHighlights,
    GoNextError,
    GoPrevError,
    SetCompact,
    SetComfortable,
    SetUltraCompact,
    ManageServers,
    ToggleFollow,
    TogglePause,
    ClearFile,
    LoadMore,
    OpenInFull,
    BackToLiveWindow,
    ToggleInspector,
    NewLogSpace,
    ToggleRawParsed,
    SaveScratch,
    ToggleProjectSwitcher,
    ToggleSidebar,
    ToggleContextSidebar,
    CloseTab,
    CloseOtherTabs,
    CommandPalette,
    OpenSettings,
    OpenKeymap,
    LoadDemo,
    UnpinAllColumns,
}

impl Command {
    /// In the order the Command Palette and the Keymap settings list them.
    pub const ALL: [Command; 35] = [
        Command::ToggleSearch,
        Command::NextMatch,
        Command::PrevMatch,
        Command::ToggleFilter,
        Command::ToggleFocusPane,
        Command::MoveFocusToWindow,
        Command::ToggleFindFilter,
        Command::ToggleSearchHighlight,
        Command::ShowHighlights,
        Command::GoNextError,
        Command::GoPrevError,
        Command::SetCompact,
        Command::SetComfortable,
        Command::SetUltraCompact,
        Command::ManageServers,
        Command::ToggleFollow,
        Command::TogglePause,
        Command::ClearFile,
        Command::LoadMore,
        Command::OpenInFull,
        Command::BackToLiveWindow,
        Command::ToggleInspector,
        Command::NewLogSpace,
        Command::ToggleRawParsed,
        Command::SaveScratch,
        Command::ToggleProjectSwitcher,
        Command::ToggleSidebar,
        Command::ToggleContextSidebar,
        Command::CloseTab,
        Command::CloseOtherTabs,
        Command::CommandPalette,
        Command::OpenSettings,
        Command::OpenKeymap,
        Command::LoadDemo,
        Command::UnpinAllColumns,
    ];

    /// The name `keymap.json` knows it by; never changes.
    pub fn id(self) -> &'static str {
        self.meta().0
    }

    /// The name shown in the Command Palette and the Keymap settings.
    pub fn name(self) -> &'static str {
        self.meta().1
    }

    /// The Shortcuts it has until the user changes them.
    pub fn default_shortcuts(self) -> &'static [&'static str] {
        self.meta().2
    }

    fn meta(self) -> (&'static str, &'static str, &'static [&'static str]) {
        match self {
            Command::ToggleSearch => ("search.find", "Search: Find in Log", &["cmd-f"]),
            Command::NextMatch => ("search.next_match", "Search: Next Match", &["cmd-g", "enter"]),
            Command::PrevMatch => ("search.prev_match", "Search: Previous Match", &["cmd-shift-g", "shift-enter"]),
            Command::ToggleFilter => ("filter.open", "Filter: Open Filter Builder", &["cmd-shift-f"]),
            Command::ToggleFocusPane => ("focus_pane.toggle", "Focus Pane: Show or Hide", &["cmd-j"]),
            Command::MoveFocusToWindow => ("focus_pane.to_window", "Focus Pane: Move to Window", &[]),
            Command::ToggleFindFilter => ("focus_pane.toggle_find_filter", "Focus Pane: Toggle Find Filter", &[]),
            Command::ToggleSearchHighlight => {
                ("focus_pane.toggle_search_highlight", "Focus Pane: Toggle Search Highlight", &[])
            }
            Command::ShowHighlights => ("highlights.open", "Highlights: Open Highlight Rules", &["cmd-shift-m"]),
            Command::GoNextError => ("navigate.next_error", "Navigate: Jump to Next Error", &["cmd-e"]),
            Command::GoPrevError => ("navigate.prev_error", "Navigate: Jump to Previous Error", &["cmd-shift-e"]),
            Command::SetCompact => ("density.compact", "Density: Compact (Default)", &[]),
            Command::SetComfortable => ("density.comfortable", "Density: Comfortable", &[]),
            Command::SetUltraCompact => ("density.ultra_compact", "Density: Ultra Compact", &[]),
            Command::ManageServers => ("servers.manage", "Servers: Manage…", &[]),
            Command::ToggleFollow => ("live.follow", "Live: Toggle Follow", &[]),
            Command::TogglePause => ("live.pause", "Live: Toggle Pause", &[]),
            Command::ClearFile => ("live.clear_file", "Live: Clear File", &["cmd-shift-k"]),
            Command::LoadMore => ("live.load_more", "Live: Load More Older Lines", &["cmd-shift-up"]),
            Command::OpenInFull => ("live.open_in_full", "Live: Open in Full…", &[]),
            Command::BackToLiveWindow => ("live.back_to_window", "Live: Back to Live Window", &[]),
            Command::ToggleInspector => ("inspector.toggle", "Inspector: Toggle Slide-out Inspector", &["cmd-i"]),
            Command::NewLogSpace => ("log_space.new", "Log Space: New Log Space…", &["cmd-o"]),
            Command::ToggleRawParsed => ("view.raw_parsed", "View: Toggle Raw / Parsed", &["cmd-shift-r"]),
            Command::SaveScratch => ("log_space.save", "Log Space: Save Scratch Files", &["cmd-s"]),
            Command::ToggleProjectSwitcher => ("project.switch", "Project: Switch Project", &["cmd-p"]),
            Command::ToggleSidebar => ("view.sidebar", "View: Show or Hide Projects Sidebar", &["cmd-b"]),
            Command::ToggleContextSidebar => ("view.context_sidebar", "View: Show or Hide Context Sidebar", &["cmd-alt-b"]),
            Command::CloseTab => ("tabs.close", "Tabs: Close Tab", &["cmd-w"]),
            Command::CloseOtherTabs => ("tabs.close_others", "Tabs: Close Other Tabs", &["cmd-alt-t"]),
            Command::CommandPalette => ("app.command_palette", "App: Command Palette", &["cmd-k"]),
            Command::OpenSettings => ("app.settings", "App: Open Settings", &["cmd-,"]),
            Command::OpenKeymap => ("app.keymap", "App: Keyboard Shortcuts", &[]),
            Command::LoadDemo => ("data.load_demo", "Data: Load Demo Log Space", &[]),
            Command::UnpinAllColumns => ("columns.unpin_all", "Columns: Unpin All", &[]),
        }
    }

    pub fn from_id(id: &str) -> Option<Command> {
        Command::ALL.into_iter().find(|c| c.id() == id)
    }
}

/// One key with the modifiers held, as the app reads it: Ctrl counts as ⌘.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub cmd: bool,
    pub alt: bool,
    pub shift: bool,
    /// Lowercase, in gpui's names: `f`, `enter`, `up`, `space`, `f5`, `,`.
    pub key: String,
}

impl Shortcut {
    /// Read `cmd-shift-f`; `ctrl` is taken as `cmd`.
    pub fn parse(text: &str) -> Option<Shortcut> {
        let mut shortcut = Shortcut { cmd: false, alt: false, shift: false, key: String::new() };
        let text = text.trim().to_lowercase();
        // The last part is the key, so `cmd--` reads as ⌘ and `-`.
        let (mods, key) = match text.rsplit_once('-') {
            Some((mods, "")) if mods.ends_with('-') => (&mods[..mods.len() - 1], "-"),
            Some((mods, key)) => (mods, key),
            None => ("", text.as_str()),
        };
        for part in mods.split('-').filter(|p| !p.is_empty()) {
            match part {
                "cmd" | "ctrl" | "control" | "super" | "platform" => shortcut.cmd = true,
                "alt" | "option" => shortcut.alt = true,
                "shift" => shortcut.shift = true,
                _ => return None,
            }
        }
        if key.is_empty() {
            return None;
        }
        shortcut.key = normalize_key(key);
        Some(shortcut)
    }

    /// The Shortcut a key press makes.
    pub fn from_keystroke(keystroke: &gpui::Keystroke) -> Shortcut {
        let m = keystroke.modifiers;
        Shortcut {
            cmd: m.platform || m.control,
            alt: m.alt,
            shift: m.shift,
            key: normalize_key(&keystroke.key),
        }
    }

    /// As `keymap.json` writes it: `cmd-shift-f`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        if self.cmd {
            out.push_str("cmd-");
        }
        if self.alt {
            out.push_str("alt-");
        }
        if self.shift {
            out.push_str("shift-");
        }
        out.push_str(&self.key);
        out
    }

    /// The gpui keystroke to draw it with: ⌘ on macOS, Ctrl elsewhere.
    pub fn keystroke(&self) -> gpui::Keystroke {
        let mut modifiers = gpui::Modifiers::default();
        if self.cmd {
            if cfg!(target_os = "macos") {
                modifiers.platform = true;
            } else {
                modifiers.control = true;
            }
        }
        modifiers.alt = self.alt;
        modifiers.shift = self.shift;
        gpui::Keystroke { modifiers, key: self.key.clone(), key_char: None }
    }

    /// No ⌘ or ⌥: it runs only while no text field has the cursor.
    pub fn is_plain(&self) -> bool {
        !self.cmd && !self.alt
    }

    /// Escape, arrows, copy, tab numbers and tab cycling keep their meaning.
    pub fn is_fixed(&self) -> bool {
        let key = self.key.as_str();
        match (self.cmd, self.alt) {
            (false, false) => matches!(key, "escape" | "up" | "down" | "left" | "right" | "tab" | "backspace"),
            (true, false) if !self.shift => {
                key == "c" || key == "tab" || matches!(key.parse::<u8>(), Ok(1..=9))
            }
            (true, false) => key == "tab",
            _ => false,
        }
    }

    /// A key on its own that only says which modifiers are held.
    pub fn is_modifier_only(&self) -> bool {
        matches!(
            self.key.as_str(),
            "" | "shift" | "control" | "ctrl" | "alt" | "option" | "cmd" | "platform" | "function" | "fn" | "capslock"
        )
    }
}

fn normalize_key(key: &str) -> String {
    match key {
        " " => "space".into(),
        "ArrowUp" => "up".into(),
        "ArrowDown" => "down".into(),
        "ArrowLeft" => "left".into(),
        "ArrowRight" => "right".into(),
        "Enter" | "return" => "enter".into(),
        "Escape" | "esc" => "escape".into(),
        other => other.to_lowercase(),
    }
}

/// Why a Shortcut was not given to a Command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignError {
    /// One of the Fixed Keys.
    Fixed,
    /// A modifier key pressed on its own.
    ModifierOnly,
}

/// The Shortcuts of every Command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keymap {
    /// The Commands the user changed, by id, with all their Shortcuts.
    changed: BTreeMap<&'static str, Vec<Shortcut>>,
}

#[derive(Serialize, Deserialize, Default)]
struct KeymapFile {
    #[serde(default)]
    shortcuts: BTreeMap<String, Vec<String>>,
}

impl Keymap {
    /// The file the Keymap is kept in when there is no session beside it.
    pub fn default_path() -> PathBuf {
        crate::workspace::session::app_data_dir().join("keymap.json")
    }

    /// The Keymap at `path`: the defaults when there is no file yet. Unknown
    /// Commands and unreadable Shortcuts in it are skipped; a file that is not
    /// a keymap at all is an error, and the defaults are used.
    pub fn load(path: &Path) -> Result<Keymap, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Keymap::default()),
            Err(e) => return Err(e.to_string()),
        };
        let file: KeymapFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let mut keymap = Keymap::default();
        for (id, shortcuts) in file.shortcuts {
            let Some(command) = Command::from_id(&id) else { continue };
            let shortcuts = shortcuts
                .iter()
                .filter_map(|s| Shortcut::parse(s))
                .filter(|s| !s.is_fixed() && !s.is_modifier_only())
                .collect();
            keymap.set(command, shortcuts);
        }
        Ok(keymap)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let file = KeymapFile {
            shortcuts: self
                .changed
                .iter()
                .map(|(id, list)| (id.to_string(), list.iter().map(Shortcut::text).collect()))
                .collect(),
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?)
    }

    pub fn shortcuts(&self, command: Command) -> Vec<Shortcut> {
        match self.changed.get(command.id()) {
            Some(list) => list.clone(),
            None => command.default_shortcuts().iter().filter_map(|s| Shortcut::parse(s)).collect(),
        }
    }

    /// The Command a key press runs, if any.
    pub fn command_for(&self, shortcut: &Shortcut) -> Option<Command> {
        Command::ALL.into_iter().find(|&c| self.shortcuts(c).contains(shortcut))
    }

    pub fn is_default(&self, command: Command) -> bool {
        !self.changed.contains_key(command.id())
    }

    pub fn has_changes(&self) -> bool {
        !self.changed.is_empty()
    }

    /// Give `command` exactly `shortcuts`.
    fn set(&mut self, command: Command, shortcuts: Vec<Shortcut>) {
        let defaults: Vec<Shortcut> =
            command.default_shortcuts().iter().filter_map(|s| Shortcut::parse(s)).collect();
        if shortcuts == defaults {
            self.changed.remove(command.id());
        } else {
            self.changed.insert(command.id(), shortcuts);
        }
    }

    /// Add `shortcut` to `command`. A Command that had it loses it, and is
    /// returned so the user can be told.
    pub fn add(&mut self, command: Command, shortcut: Shortcut) -> Result<Option<Command>, AssignError> {
        if shortcut.is_modifier_only() {
            return Err(AssignError::ModifierOnly);
        }
        if shortcut.is_fixed() {
            return Err(AssignError::Fixed);
        }
        let previous = self.command_for(&shortcut);
        if previous == Some(command) {
            return Ok(None);
        }
        if let Some(other) = previous {
            let rest = self.shortcuts(other).into_iter().filter(|s| *s != shortcut).collect();
            self.set(other, rest);
        }
        let mut list = self.shortcuts(command);
        list.push(shortcut);
        self.set(command, list);
        Ok(previous)
    }

    pub fn remove(&mut self, command: Command, shortcut: &Shortcut) {
        let rest = self.shortcuts(command).into_iter().filter(|s| s != shortcut).collect();
        self.set(command, rest);
    }

    /// Back to its default Shortcuts. A Command that took one of them since
    /// gives it back.
    pub fn reset(&mut self, command: Command) {
        self.changed.remove(command.id());
        for shortcut in self.shortcuts(command) {
            for other in Command::ALL.into_iter().filter(|&c| c != command) {
                if self.shortcuts(other).contains(&shortcut) {
                    self.remove(other, &shortcut);
                }
            }
        }
    }

    pub fn reset_all(&mut self) {
        self.changed.clear();
    }
}
