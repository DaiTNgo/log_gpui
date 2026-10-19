//! Remote Command: the text of a command run on a Server, the check that it
//! does not elevate, and the Command Presets that write the text for the
//! common log commands (journalctl, Docker, Kubernetes).

use serde::{Deserialize, Serialize};

/// `'text'` with every quote inside made safe for a shell.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// No elevation (L9)
// ---------------------------------------------------------------------------

/// Commands that give another user's rights, refused as the first word of a
/// part of the command.
const ELEVATORS: [&str; 5] = ["sudo", "su", "doas", "pkexec", "run0"];

/// Words that only run the next word, skipped when looking for the command.
const PREFIXES: [&str; 4] = ["command", "exec", "nohup", "time"];

/// The command starts something that gives other rights (`sudo`, `su`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elevation {
    /// The word found, as typed (`/usr/bin/sudo`).
    pub word: String,
}

/// Why `command` cannot be a Remote Command.
pub fn check_elevation(command: &str) -> Result<(), Elevation> {
    for part in split_parts(command) {
        let mut words = part.into_iter().peekable();
        loop {
            let Some(word) = words.next() else { break };
            if is_assignment(&word) {
                continue;
            }
            let base = word.rsplit('/').next().unwrap_or(&word);
            if ELEVATORS.contains(&base) {
                return Err(Elevation { word });
            }
            if base == "env" || PREFIXES.contains(&base) {
                // `env -i FOO=1 sudo x`, `command -p su`: skip the options.
                while words.peek().is_some_and(|w| w.starts_with('-')) {
                    words.next();
                }
                continue;
            }
            break;
        }
    }
    Ok(())
}

fn is_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty()
                && !name.starts_with(|c: char| c.is_ascii_digit())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false,
    }
}

/// The words of each part of `command`; parts end at `|`, `;`, `&`, a
/// newline, a parenthesis or a backtick, outside quotes.
fn split_parts(command: &str) -> Vec<Vec<String>> {
    let mut parts: Vec<Vec<String>> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = command.chars().peekable();
    let end_word = |word: &mut String, in_word: &mut bool, words: &mut Vec<String>| {
        if *in_word {
            words.push(std::mem::take(word));
            *in_word = false;
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                in_word = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => {
                            if let Some(next) = chars.next() {
                                word.push(next);
                            }
                        }
                        _ => word.push(q),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            c if c.is_whitespace() && c != '\n' => end_word(&mut word, &mut in_word, &mut words),
            '|' | ';' | '&' | '\n' | '(' | ')' | '`' => {
                end_word(&mut word, &mut in_word, &mut words);
                if !words.is_empty() {
                    parts.push(std::mem::take(&mut words));
                }
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
    }
    end_word(&mut word, &mut in_word, &mut words);
    if !words.is_empty() {
        parts.push(words);
    }
    parts
}

// ---------------------------------------------------------------------------
// Command Presets
// ---------------------------------------------------------------------------

/// Lines shown first when a Command Preset starts.
pub const DEFAULT_PRESET_LINES: u32 = 1_000;

/// The fields of a Command Preset. An empty text means the field is not set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresetFields {
    Journal { unit: String, lines: u32 },
    Docker { container: String, lines: u32 },
    Kube { pod: String, namespace: String, container: String, context: String, lines: u32 },
}

/// Where a Command Preset's output starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// The last `n` lines, then new ones.
    Tail(u32),
    /// From a time, in the notation of the tool (`@1760842923` for
    /// journalctl, the output's own `2026-10-19T03:02:03.123456789Z` for
    /// Docker and kubectl).
    Since(String),
}

impl PresetFields {
    pub fn journal(unit: &str) -> Self {
        PresetFields::Journal { unit: unit.to_string(), lines: DEFAULT_PRESET_LINES }
    }

    pub fn docker(container: &str) -> Self {
        PresetFields::Docker { container: container.to_string(), lines: DEFAULT_PRESET_LINES }
    }

    pub fn kube(pod: &str, namespace: &str, container: &str, context: &str) -> Self {
        PresetFields::Kube {
            pod: pod.to_string(),
            namespace: namespace.to_string(),
            container: container.to_string(),
            context: context.to_string(),
            lines: DEFAULT_PRESET_LINES,
        }
    }

    pub fn lines(&self) -> u32 {
        match self {
            PresetFields::Journal { lines, .. }
            | PresetFields::Docker { lines, .. }
            | PresetFields::Kube { lines, .. } => *lines,
        }
    }

    /// The command that starts at `start`.
    pub fn render(&self, start: &Start) -> String {
        render(self, start)
    }

    /// The command as added, with the initial lines.
    pub fn command(&self) -> String {
        render(self, &Start::Tail(self.lines()))
    }
}

/// The command a Command Preset writes.
pub fn render(fields: &PresetFields, start: &Start) -> String {
    let mut command = String::new();
    match fields {
        PresetFields::Journal { unit, .. } => {
            command.push_str("journalctl --no-pager -o short-iso-precise -f");
            match start {
                Start::Tail(n) => command.push_str(&format!(" -n {n}")),
                Start::Since(ts) => command.push_str(&format!(" --since {}", since_value(ts))),
            }
            if !unit.is_empty() {
                command.push_str(&format!(" -u {}", shell_quote(unit)));
            }
        }
        PresetFields::Docker { container, .. } => {
            command.push_str("docker logs -f --timestamps");
            match start {
                Start::Tail(n) => command.push_str(&format!(" --tail {n}")),
                Start::Since(ts) => command.push_str(&format!(" --since {}", since_value(ts))),
            }
            command.push(' ');
            command.push_str(&shell_quote(container));
        }
        PresetFields::Kube { pod, namespace, container, context, .. } => {
            command.push_str("kubectl logs -f --timestamps");
            match start {
                Start::Tail(n) => command.push_str(&format!(" --tail={n}")),
                Start::Since(ts) => command.push_str(&format!(" --since-time={}", since_value(ts))),
            }
            if !context.is_empty() {
                command.push_str(&format!(" --context {}", shell_quote(context)));
            }
            if !namespace.is_empty() {
                command.push_str(&format!(" -n {}", shell_quote(namespace)));
            }
            if !container.is_empty() {
                command.push_str(&format!(" -c {}", shell_quote(container)));
            }
            command.push(' ');
            command.push_str(&shell_quote(pod));
        }
    }
    command
}

/// A time as the flag takes it: as is when it is made of safe characters
/// (always, for what comes from output), quoted otherwise.
fn since_value(ts: &str) -> String {
    let safe = !ts.is_empty()
        && ts.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '+' | '-' | '@' | '_'));
    if safe { ts.to_string() } else { shell_quote(ts) }
}

/// What to keep of a preset for a command as stored: the fields only when the
/// command is exactly what they write (an edited command is typed by hand).
pub fn unedited_preset(command: &str, preset: Option<&PresetFields>) -> Option<PresetFields> {
    let preset = preset?;
    (render(preset, &Start::Tail(preset.lines())) == command).then(|| preset.clone())
}
