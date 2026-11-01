//! Telling a Rotated File by its name: `<name>.<N>`, `<name>-<YYYYMMDD>` or
//! `<name>-<YYYY-MM-DD>`, with or without a compression suffix. It is an
//! earlier Generation of the file `<name>`, not a file of its own. The same
//! rule gives the Log Name of a Source File (openspec change `add-swimlane`).

const COMPRESSION: [&str; 5] = ["gz", "bz2", "xz", "zst", "Z"];

/// Where a rotation sits in the sequence of its file, by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationKey {
    /// `<name>.<N>`: a bigger number is older.
    Number(u64),
    /// A date as `YYYYMMDDhhmmss` (a date alone has zeros for the time): a
    /// bigger date is newer.
    Date(u64),
}

impl RotationKey {
    /// Ascending is newest first.
    pub(crate) fn newest_first(self) -> i128 {
        match self {
            RotationKey::Number(n) => n as i128,
            RotationKey::Date(d) => -(d as i128),
        }
    }
}

/// The name a Rotated File was rotated from, by its name alone. `None` when
/// the name does not look like a rotation of anything.
pub fn rotated_base(name: &str) -> Option<&str> {
    rotation(name).map(|(base, _)| base)
}

/// The name a Rotated File was rotated from, and where it sits in the
/// sequence.
pub fn rotation(name: &str) -> Option<(&str, RotationKey)> {
    let mut stem = name;
    if let Some((rest, ext)) = stem.rsplit_once('.') {
        if COMPRESSION.contains(&ext) {
            stem = rest;
        }
    }
    let (base, key) = strip_rotation_suffix(stem)?;
    (!base.is_empty()).then_some((base, key))
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `stem` without its `.<N>` or date suffix, and where that puts it in the
/// sequence; `None` when it has none.
pub fn strip_rotation_suffix(stem: &str) -> Option<(&str, RotationKey)> {
    // `<name>.<N>`
    if let Some((base, n)) = stem.rsplit_once('.') {
        if all_digits(n) {
            return Some((base, RotationKey::Number(n.parse().ok()?)));
        }
    }
    // `<name>-<YYYY-MM-DD>` or `<name>.<YYYY-MM-DD>`
    if stem.len() > 11 {
        let (base, tail) = stem.split_at(stem.len() - 11);
        let date = tail.strip_prefix('-').or_else(|| tail.strip_prefix('.'));
        if let Some(date) = date {
            let parts: Vec<&str> = date.split('-').collect();
            if parts.len() == 3
                && parts[0].len() == 4
                && parts[1].len() == 2
                && parts[2].len() == 2
                && parts.iter().all(|p| all_digits(p))
            {
                let day: u64 = parts.concat().parse().ok()?;
                return Some((base, RotationKey::Date(day * 1_000_000)));
            }
        }
    }
    // `<name>-<YYYYMMDD>` or `<name>-<YYYYMMDDhhmmss>`
    for len in [14usize, 8] {
        if stem.len() > len + 1 {
            let (base, tail) = stem.split_at(stem.len() - len - 1);
            if let Some(date) = tail.strip_prefix('-') {
                if date.len() == len && all_digits(date) {
                    let mut value: u64 = date.parse().ok()?;
                    if len == 8 {
                        value *= 1_000_000;
                    }
                    return Some((base, RotationKey::Date(value)));
                }
            }
        }
    }
    None
}


/// The Log Name of a Source File called `source`: its file name without a
/// compression suffix and then a rotation suffix, by the rule above. What
/// stands before the file name (a folder, `<server>:`, `<archive> › `) is
/// kept. A name that is not a rotation stays whole, compression suffix and
/// all.
pub fn log_name(source: &str) -> &str {
    let name_at = [
        source.rfind('/').map(|i| i + 1),
        source.rfind(':').map(|i| i + 1),
        source.rfind(" › ").map(|i| i + " › ".len()),
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or(0);
    match rotated_base(&source[name_at..]) {
        // The base is the start of the file name.
        Some(base) => &source[..name_at + base.len()],
        None => source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_files_share_a_log_name() {
        for name in ["message", "message.0", "message.1"] {
            assert_eq!(log_name(name), "message");
        }
    }

    #[test]
    fn compression_and_rotation_both_go() {
        for name in ["db.log.1.gz", "db.log.2", "db.log-20261029"] {
            assert_eq!(log_name(name), "db.log");
        }
    }

    #[test]
    fn what_stands_before_the_name_is_kept() {
        assert_eq!(log_name("web1:/var/log/app.log.1"), "web1:/var/log/app.log");
        assert_eq!(log_name("web2:/var/log/app.log"), "web2:/var/log/app.log");
        assert_eq!(log_name("/var/log/app.log-2026-10-19.gz"), "/var/log/app.log");
        assert_eq!(log_name("web1:app.log.3"), "web1:app.log");
    }

    #[test]
    fn names_that_are_not_rotations_stay_whole() {
        assert_eq!(log_name("api.log"), "api.log");
        assert_eq!(log_name("trace.gz"), "trace.gz");
        assert_eq!(log_name("/var/log/"), "/var/log/");
        assert_eq!(log_name(""), "");
    }

    #[test]
    fn archive_entries_split_at_the_last_marker() {
        assert_eq!(log_name("logs-2026.tar.gz › app.log.1"), "logs-2026.tar.gz › app.log");
        assert_eq!(log_name("logs.tar.gz › var/log/db.log.2.gz"), "logs.tar.gz › var/log/db.log");
        assert_eq!(log_name("logs.zip › notes.txt"), "logs.zip › notes.txt");
    }
}
