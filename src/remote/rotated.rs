//! Telling a Rotated File by its name: `<name>.<N>`, `<name>-<YYYYMMDD>` or
//! `<name>-<YYYY-MM-DD>`, with or without a compression suffix. It is an
//! earlier Generation of the file `<name>`, not a file of its own.

const COMPRESSION: [&str; 5] = ["gz", "bz2", "xz", "zst", "Z"];

/// The name a Rotated File was rotated from, by its name alone. `None` when
/// the name does not look like a rotation of anything.
pub fn rotated_base(name: &str) -> Option<&str> {
    let mut stem = name;
    if let Some((rest, ext)) = stem.rsplit_once('.') {
        if COMPRESSION.contains(&ext) {
            stem = rest;
        }
    }
    let base = strip_rotation_suffix(stem)?;
    (!base.is_empty()).then_some(base)
}

fn strip_rotation_suffix(stem: &str) -> Option<&str> {
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    // `<name>.<N>`
    if let Some((base, n)) = stem.rsplit_once('.') {
        if all_digits(n) {
            return Some(base);
        }
    }
    // `<name>-<YYYY-MM-DD>`
    if stem.len() > 11 {
        let (base, tail) = stem.split_at(stem.len() - 11);
        if let Some(date) = tail.strip_prefix('-') {
            let parts: Vec<&str> = date.split('-').collect();
            if parts.len() == 3
                && parts[0].len() == 4
                && parts[1].len() == 2
                && parts[2].len() == 2
                && parts.iter().all(|p| all_digits(p))
            {
                return Some(base);
            }
        }
    }
    // `<name>-<YYYYMMDD>`
    if stem.len() > 9 {
        let (base, tail) = stem.split_at(stem.len() - 9);
        if let Some(date) = tail.strip_prefix('-') {
            if date.len() == 8 && all_digits(date) {
                return Some(base);
            }
        }
    }
    None
}

/// Whether `name` is a Rotated File of a file for which `has_base` says yes.
pub fn is_rotated_file(name: &str, has_base: impl Fn(&str) -> bool) -> bool {
    rotated_base(name).is_some_and(has_base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_and_dated_rotations() {
        assert_eq!(rotated_base("app.log.1"), Some("app.log"));
        assert_eq!(rotated_base("app.log.12"), Some("app.log"));
        assert_eq!(rotated_base("app.log-20261019"), Some("app.log"));
        assert_eq!(rotated_base("app.log-2026-10-19"), Some("app.log"));
    }

    #[test]
    fn compressed_rotations() {
        assert_eq!(rotated_base("app.log.2.gz"), Some("app.log"));
        assert_eq!(rotated_base("app.log-20261019.xz"), Some("app.log"));
        assert_eq!(rotated_base("app.log.3.zst"), Some("app.log"));
    }

    #[test]
    fn plain_names_are_not_rotations() {
        assert_eq!(rotated_base("app.log"), None);
        assert_eq!(rotated_base("app-2026-10-19.log"), None);
        assert_eq!(rotated_base("dump.tar.gz"), None);
        assert_eq!(rotated_base("app.log.gz"), None);
        assert_eq!(rotated_base("2026"), None);
        assert_eq!(rotated_base(".1"), None);
    }

    #[test]
    fn needs_the_base_to_exist() {
        let names = ["app.log", "app.log.1"];
        let has = |b: &str| names.contains(&b);
        assert!(is_rotated_file("app.log.1", has));
        assert!(!is_rotated_file("other.log.1", has));
    }
}
