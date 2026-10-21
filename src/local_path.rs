//! Local filesystem paths as text: file name, folder, absoluteness.
//!
//! Splitting is by path rules, not by `/`: on Windows both `\` and `/`
//! separate and `C:\` is a root. The rules are chosen by [`PathStyle`] so the
//! Windows ones can be tested on any platform. Remote paths always split on
//! `/` and do not use this module.

/// Which platform's rules a local path follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStyle {
    Unix,
    Windows,
}

impl PathStyle {
    /// The style of the running platform.
    pub const fn native() -> Self {
        if cfg!(windows) { Self::Windows } else { Self::Unix }
    }

    fn is_sep(self, c: char) -> bool {
        c == '/' || (self == Self::Windows && c == '\\')
    }
}

/// `path` cut at its last separator: (folder part, name, separator used).
/// The folder part keeps a root whole (`C:\`, `/`). `None` without separator.
pub fn split_last(path: &str, style: PathStyle) -> Option<(&str, &str, char)> {
    let (at, sep) = path.char_indices().rfind(|&(_, c)| style.is_sep(c))?;
    let (dir, name) = (&path[..at], &path[at + 1..]);
    let is_root = dir.is_empty() || (style == PathStyle::Windows && dir.len() == 2 && dir.ends_with(':'));
    Some((&path[..if is_root { at + 1 } else { at }], name, sep))
}

/// `rest` (written with `/`) appended to `base`, using `base`'s own
/// separator: a Windows base (`C:\Users\me`) gets `\` throughout, so the
/// result never mixes the two. Any other base is joined with `/`.
pub fn join_like(base: &str, rest: &str) -> String {
    if base.contains('\\') {
        format!("{}\\{}", base.trim_end_matches('\\'), rest.replace('/', "\\"))
    } else {
        format!("{base}/{rest}")
    }
}

/// The last component of `path` (`None` when it ends in a separator).
pub fn file_name(path: &str, style: PathStyle) -> Option<&str> {
    let name = split_last(path, style).map_or(path, |(_, name, _)| name);
    (!name.is_empty()).then_some(name)
}

/// The folder holding `path`, if it names one.
pub fn folder(path: &str, style: PathStyle) -> Option<String> {
    split_last(path, style).map(|(dir, _, _)| dir.to_string()).filter(|d| !d.is_empty())
}

/// Whether `path` is absolute (`C:\x`, `C:/x`, `\\srv\share` and `/x` on
/// Windows; `/x` elsewhere).
pub fn is_absolute(path: &str, style: PathStyle) -> bool {
    match style {
        PathStyle::Unix => path.starts_with('/'),
        PathStyle::Windows => {
            let b = path.as_bytes();
            path.starts_with(['/', '\\'])
                || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\'))
        }
    }
}
