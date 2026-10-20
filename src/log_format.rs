//! Log Formats: a named template that describes a whole line with Format
//! Tokens (`%{IP:client}`), compiled to a regex. The fields `timestamp`,
//! `level`, `module` and `message` fill the fixed columns; every other field
//! is a Field Column, in template order. A line the format does not match is
//! read by Auto-detect (rule 39). Log Formats are a library of the app, like
//! Servers; which one a Log Space or Source File uses belongs to the Project.

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// The fields that fill fixed columns rather than Field Columns.
pub const FIXED_FIELDS: [&str; 4] = ["timestamp", "level", "module", "message"];

/// The Format Tokens and the regex each stands for.
pub const TOKENS: [(&str, &str); 12] = [
    ("TIMESTAMP", TIMESTAMP_RE),
    ("LEVEL", r"(?i:trace|debug|info|notice|warn(?:ing)?|error|err|crit(?:ical)?|fatal|severe|[VDIWEF])"),
    ("WORD", r"\w+"),
    ("NOTSPACE", r"\S+"),
    ("INT", r"[+-]?\d+"),
    ("NUMBER", r"[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?"),
    ("IP", r"(?:\d{1,3}(?:\.\d{1,3}){3}|[0-9A-Fa-f]*:[0-9A-Fa-f:.]+)"),
    ("UUID", r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"),
    ("HEX", r"(?:0[xX])?[0-9A-Fa-f]+"),
    ("QUOTED", r#""(?:[^"\\]|\\.)*""#),
    ("DATA", r".*?"),
    ("GREEDY", r".*"),
];

/// Every layout `read_timestamp` understands: ISO-like (with `.` or `,`
/// before the fraction), the time of nginx/Apache, logcat and syslog.
const TIMESTAMP_RE: &str = concat!(
    r"(?:\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:\s?(?:Z|[+-]\d{2}:?\d{2}))?",
    r"|\d{2}/[A-Z][a-z]{2}/\d{4}:\d{2}:\d{2}:\d{2}(?:\s[+-]\d{4})?",
    r"|\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d+",
    r"|[A-Z][a-z]{2}\s+\d+\s+\d{2}:\d{2}:\d{2})"
);

/// Most memory a compiled template may take.
const SIZE_LIMIT: usize = 2 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogFormat {
    pub id: String,
    pub name: String,
    pub template: String,
    /// File name globs (`access*.log`) the format applies to on its own.
    #[serde(default)]
    pub globs: Vec<String>,
}

/// Why a template cannot be used, and which part of it is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError {
    /// One sentence saying how to fix it.
    pub message: String,
    /// The wrong part, in bytes of the template.
    pub span: Range<usize>,
}

/// A template compiled to a regex, with its fields in template order.
#[derive(Debug, Clone)]
pub struct CompiledFormat {
    pub regex: Regex,
    /// Every field the template names, in order.
    pub fields: Vec<String>,
}

impl CompiledFormat {
    /// The fields that become Field Columns (not the fixed ones), in order.
    pub fn column_fields(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(String::as_str).filter(|f| !FIXED_FIELDS.contains(f))
    }

    /// Where each field is in `line`, or `None` when the format does not
    /// match it. The template matches the line from its first character that
    /// is not whitespace to its end.
    pub fn read(&self, line: &str) -> Option<HashMap<String, Range<usize>>> {
        let lead = line.len() - line.trim_start().len();
        let caps = self.regex.captures(&line[lead..])?;
        let mut spans = HashMap::new();
        for name in &self.fields {
            if let Some(m) = caps.name(name) {
                spans.insert(name.clone(), m.start() + lead..m.end() + lead);
            }
        }
        Some(spans)
    }
}

fn is_field_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The end (exclusive) of the group that opens at `start`, skipping escapes
/// and character classes.
fn group_end(t: &str, start: usize) -> Option<usize> {
    let b = t.as_bytes();
    let (mut depth, mut i, mut class) = (0usize, start, false);
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'[' if !class => class = true,
            b']' if class => class = false,
            b'(' if !class => depth += 1,
            b')' if !class => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Compile a template. `%{TOKEN}` matches without reading, `%{TOKEN:field}`
/// reads a field, a named group `(?<field>…)` is raw regex read as a field,
/// a run of whitespace matches one or more whitespace characters, and every
/// other character matches itself.
pub fn compile_template(template: &str) -> Result<CompiledFormat, TemplateError> {
    if template.trim().is_empty() {
        return Err(TemplateError { message: "Type a template, like %{TIMESTAMP:timestamp} %{GREEDY:message}".into(), span: 0..0 });
    }
    let mut regex = String::from("^");
    let mut fields: Vec<String> = Vec::new();
    let add_field = |name: &str, span: Range<usize>, fields: &mut Vec<String>| -> Result<(), TemplateError> {
        if !is_field_name(name) {
            return Err(TemplateError {
                message: format!("\"{name}\" is not a field name — use letters, digits and _"),
                span,
            });
        }
        if fields.iter().any(|f| f == name) {
            return Err(TemplateError { message: format!("The field \"{name}\" is read twice — rename one"), span });
        }
        fields.push(name.to_string());
        Ok(())
    };
    let mut i = 0;
    while i < template.len() {
        let rest = &template[i..];
        if rest.starts_with("%{") {
            let Some(close) = rest.find('}') else {
                return Err(TemplateError {
                    message: "This Format Token is not closed — add }".into(),
                    span: i..template.len(),
                });
            };
            let inner = &rest[2..close];
            let (token, field) = match inner.split_once(':') {
                Some((t, f)) => (t, Some(f)),
                None => (inner, None),
            };
            let Some((_, re)) = TOKENS.iter().find(|(t, _)| *t == token) else {
                let names: Vec<&str> = TOKENS.iter().map(|(t, _)| *t).collect();
                return Err(TemplateError {
                    message: format!("{token} is not a Format Token — use one of {}", names.join(", ")),
                    span: i + 2..i + 2 + token.len(),
                });
            };
            match field {
                Some(f) => {
                    add_field(f, i + 3 + token.len()..i + close, &mut fields)?;
                    regex.push_str(&format!("(?P<{f}>{re})"));
                }
                None => regex.push_str(&format!("(?:{re})")),
            }
            i += close + 1;
        } else if rest.starts_with("(?<") || rest.starts_with("(?P<") {
            let Some(end) = group_end(template, i) else {
                return Err(TemplateError { message: "This group is not closed — add )".into(), span: i..template.len() });
            };
            let name_start = i + if rest.starts_with("(?P<") { 4 } else { 3 };
            let Some(name_len) = template[name_start..end].find('>') else {
                return Err(TemplateError { message: "Name the group: (?<field>…)".into(), span: i..end });
            };
            let name = &template[name_start..name_start + name_len];
            add_field(name, name_start..name_start + name_len, &mut fields)?;
            let body = &template[name_start + name_len + 1..end - 1];
            if let Err(e) = Regex::new(body) {
                let last = e.to_string().lines().filter(|l| !l.trim().is_empty()).next_back().unwrap_or("").trim().to_string();
                return Err(TemplateError {
                    message: format!("Not a valid pattern: {}", last.trim_start_matches("error: ")),
                    span: i..end,
                });
            }
            regex.push_str(&format!("(?P<{name}>{body})"));
            i = end;
        } else {
            let ch = rest.chars().next().unwrap();
            if ch.is_whitespace() {
                let run = rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len());
                regex.push_str(r"\s+");
                i += run;
            } else {
                regex.push_str(&regex::escape(&ch.to_string()));
                i += ch.len_utf8();
            }
        }
    }
    regex.push_str(r"\s*$");
    let regex = RegexBuilder::new(&regex).size_limit(SIZE_LIMIT).build().map_err(|e| TemplateError {
        message: match e {
            regex::Error::CompiledTooBig(_) => "The template is too large — make it shorter".into(),
            other => format!("Not a valid template: {other}"),
        },
        span: 0..template.len(),
    })?;
    Ok(CompiledFormat { regex, fields })
}

// ─── Built-in templates ──────────────────────────────────────────────────────

/// Templates to copy into the editor and adjust: (name, template, globs).
pub const BUILT_IN: [(&str, &str, &str); 4] = [
    (
        "nginx / Apache combined",
        r#"%{IP:client} - %{NOTSPACE:user} [%{TIMESTAMP:timestamp}] "%{DATA:request}" %{INT:status} %{INT:bytes} %{QUOTED:referrer} %{QUOTED:agent}"#,
        "access*.log",
    ),
    ("Python logging (default)", "%{LEVEL:level}:%{DATA:module}:%{GREEDY:message}", ""),
    (
        "Python logging (asctime)",
        "%{TIMESTAMP:timestamp} - %{DATA:module} - %{LEVEL:level} - %{GREEDY:message}",
        "",
    ),
    (
        "log4j PatternLayout (TTCC)",
        "%{INT:elapsed} [%{DATA:thread}] %{LEVEL:level} %{NOTSPACE:module} %{DATA:ndc}- %{GREEDY:message}",
        "",
    ),
];

// ─── Which format a file uses ────────────────────────────────────────────────

/// A choice made by hand for a Log Space or a Source File.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatChoice {
    /// Read as if there were no Log Format.
    AutoDetect,
    /// The Log Format of this id.
    Format(String),
}

/// The Log Format choices of a Project, by Log Space and by Source File.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatChoices {
    #[serde(default)]
    pub spaces: HashMap<String, FormatChoice>,
    #[serde(default)]
    pub files: HashMap<String, FormatChoice>,
}

impl FormatChoices {
    /// Forget the choices of a Log Format that no longer exists.
    pub fn forget(&mut self, format_id: &str) {
        let gone = FormatChoice::Format(format_id.to_string());
        self.spaces.retain(|_, c| *c != gone);
        self.files.retain(|_, c| *c != gone);
    }
}

/// Whether `name` matches the glob `glob` (`*` any run, `?` one character).
pub fn glob_matches(glob: &str, name: &str) -> bool {
    fn go(g: &[char], n: &[char]) -> bool {
        match (g.first(), n.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&g[1..], n) || (!n.is_empty() && go(g, &n[1..])),
            (Some('?'), Some(_)) => go(&g[1..], &n[1..]),
            (Some(a), Some(b)) => a == b && go(&g[1..], &n[1..]),
            _ => false,
        }
    }
    let (g, n): (Vec<char>, Vec<char>) = (glob.chars().collect(), name.chars().collect());
    go(&g, &n)
}

/// The Log Format of a Source File: chosen for the file, else for its Log
/// Space, else the first in `library` with a glob matching the file's name
/// (never its path), else none (Auto-detect).
pub fn resolve<'a>(
    library: &'a [LogFormat],
    choices: &FormatChoices,
    space_id: &str,
    file_id: &str,
    file_name: &str,
) -> Option<&'a LogFormat> {
    let by_id = |id: &str| library.iter().find(|f| f.id == id);
    match choices.files.get(file_id).or_else(|| choices.spaces.get(space_id)) {
        Some(FormatChoice::Format(id)) => return by_id(id),
        Some(FormatChoice::AutoDetect) => return None,
        None => {}
    }
    let name = Path::new(file_name).file_name().and_then(|n| n.to_str()).unwrap_or(file_name);
    library.iter().find(|f| f.globs.iter().any(|g| glob_matches(g.trim(), name)))
}

// ─── The library ─────────────────────────────────────────────────────────────

/// The Log Formats of the app, saved beside the Servers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogFormatStore {
    #[serde(default)]
    pub formats: Vec<LogFormat>,
}

impl LogFormatStore {
    /// The file the library is kept in.
    pub fn default_path() -> PathBuf {
        crate::workspace::session::app_data_dir().join("log_formats.json")
    }

    /// The library at `path`; empty when there is none yet or it cannot be read.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).map_err(std::io::Error::other)?)
    }

    pub fn get(&self, id: &str) -> Option<&LogFormat> {
        self.formats.iter().find(|f| f.id == id)
    }

    fn fresh_id(&self) -> String {
        let n = (1..).find(|n| self.get(&format!("fmt-{n}")).is_none()).unwrap_or(1);
        format!("fmt-{n}")
    }

    /// Add a format; returns its id.
    pub fn add(&mut self, name: &str, template: &str, globs: Vec<String>) -> String {
        let id = self.fresh_id();
        self.formats.push(LogFormat { id: id.clone(), name: name.trim().to_string(), template: template.to_string(), globs });
        id
    }

    pub fn update(&mut self, id: &str, name: &str, template: &str, globs: Vec<String>) {
        if let Some(f) = self.formats.iter_mut().find(|f| f.id == id) {
            f.name = name.trim().to_string();
            f.template = template.to_string();
            f.globs = globs;
        }
    }

    /// A copy named "<name> copy", right after the original; returns its id.
    pub fn duplicate(&mut self, id: &str) -> Option<String> {
        let at = self.formats.iter().position(|f| f.id == id)?;
        let mut copy = self.formats[at].clone();
        copy.id = self.fresh_id();
        copy.name = format!("{} copy", copy.name);
        let new_id = copy.id.clone();
        self.formats.insert(at + 1, copy);
        Some(new_id)
    }

    pub fn remove(&mut self, id: &str) {
        self.formats.retain(|f| f.id != id);
    }
}

// ─── Preview ─────────────────────────────────────────────────────────────────

/// A line of the editor's preview.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatPreviewLine {
    /// Position of the line among the view's entries.
    pub entry: usize,
    pub text: String,
    /// Each field read, with the position of its name among the template's
    /// fields (which picks its color).
    pub fields: Vec<(Range<usize>, usize)>,
    pub matched: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormatPreview {
    pub lines: Vec<FormatPreviewLine>,
    /// Field names, in template order (their colors follow it).
    pub field_names: Vec<String>,
    /// The first line of the view the template does not match.
    pub first_unmatched: Option<usize>,
}

impl FormatPreview {
    pub fn matched(&self) -> usize {
        self.lines.iter().filter(|l| l.matched).count()
    }

    /// "17 of 20 lines match (85%)".
    pub fn summary(&self) -> String {
        let (m, t) = (self.matched(), self.lines.len());
        let pct = if t == 0 { 0 } else { m * 100 / t };
        format!("{m} of {t} lines match ({pct}%)")
    }
}

/// Lines a preview shows.
pub const PREVIEW_LINES: usize = 20;

/// The preview of `format` on `shown` (entry position, line), and the first
/// of `all` (entry position, line) it does not match.
pub fn preview_format<'a>(
    format: &CompiledFormat,
    shown: &[(usize, &str)],
    all: impl IntoIterator<Item = (usize, &'a str)>,
) -> FormatPreview {
    let lines = shown
        .iter()
        .map(|&(entry, line)| {
            let spans = format.read(line);
            let mut fields: Vec<(Range<usize>, usize)> = spans
                .iter()
                .flatten()
                .filter(|(_, r)| r.start < r.end)
                .filter_map(|(name, r)| Some((r.clone(), format.fields.iter().position(|f| f == name)?)))
                .collect();
            fields.sort_by_key(|(r, _)| r.start);
            FormatPreviewLine { entry, text: line.to_string(), fields, matched: spans.is_some() }
        })
        .collect();
    let first_unmatched = all.into_iter().find(|(_, l)| format.read(l).is_none()).map(|(e, _)| e);
    FormatPreview { lines, field_names: format.fields.clone(), first_unmatched }
}
