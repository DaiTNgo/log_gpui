//! Go to Line: reading what the user typed and where it lands, with no GPUI.
//!
//! A jump never changes what the view shows; this only works out which line
//! of one file it is.

/// What the user typed could not be read as a line number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoToError {
    /// Not a whole number of 1 or more.
    NotALineNumber,
    /// The file has no lines to go to.
    NoLines,
    /// A time before the first or after the last line of the view.
    TimeOutside,
}

impl GoToError {
    pub fn message(self) -> &'static str {
        match self {
            GoToError::NotALineNumber => "Type a line number, a time, or file:line",
            GoToError::NoLines => "This file has no lines",
            GoToError::TimeOutside => "That time is outside the view",
        }
    }
}

/// Read a typed line number. Digit groups may be split by `,`, `_` or
/// spaces (`18 422`, `18,422`), the way line numbers get copied from reports.
pub fn parse_line(text: &str) -> Result<usize, GoToError> {
    let digits: String = text
        .trim()
        .chars()
        .filter(|c| !matches!(c, ',' | '_') && !c.is_whitespace())
        .collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(GoToError::NotALineNumber);
    }
    match digits.parse::<usize>() {
        Ok(0) => Err(GoToError::NotALineNumber),
        Ok(n) => Ok(n),
        // More digits than any file has lines: past the end.
        Err(_) => Ok(usize::MAX),
    }
}

/// The line a Go to Line lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineTarget {
    /// 1-based line of the file.
    pub line: usize,
    /// The typed number was past the last line, so this is the last line.
    pub past_end: bool,
}

/// Where line `n` of a file of `line_count` lines lands: on it, or on the
/// last line when the file is shorter.
pub fn resolve_line(line_count: usize, n: usize) -> Result<LineTarget, GoToError> {
    if line_count == 0 {
        return Err(GoToError::NoLines);
    }
    Ok(LineTarget {
        line: n.min(line_count),
        past_end: n > line_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_and_grouped_numbers() {
        assert_eq!(parse_line("42"), Ok(42));
        assert_eq!(parse_line("  7 \n"), Ok(7));
        assert_eq!(parse_line("18 422"), Ok(18422));
        assert_eq!(parse_line("18,422"), Ok(18422));
        assert_eq!(parse_line("1_000"), Ok(1000));
    }

    #[test]
    fn refuses_what_is_not_a_line_number() {
        for text in ["", "   ", "0", "-3", "+3", "12a", "1.5", "abc", "1e3"] {
            assert_eq!(parse_line(text), Err(GoToError::NotALineNumber), "{text:?}");
        }
    }

    #[test]
    fn a_huge_number_is_past_the_end() {
        assert_eq!(parse_line("99999999999999999999999999"), Ok(usize::MAX));
        assert_eq!(resolve_line(10, usize::MAX), Ok(LineTarget { line: 10, past_end: true }));
    }

    #[test]
    fn lands_on_the_line_or_the_last_one() {
        assert_eq!(resolve_line(10, 1), Ok(LineTarget { line: 1, past_end: false }));
        assert_eq!(resolve_line(10, 10), Ok(LineTarget { line: 10, past_end: false }));
        assert_eq!(resolve_line(10, 11), Ok(LineTarget { line: 10, past_end: true }));
        assert_eq!(resolve_line(0, 1), Err(GoToError::NoLines));
    }
}

/// A key that moves between the rows of the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKey {
    Down,
    Up,
    PageDown,
    PageUp,
    Home,
    End,
}

impl RowKey {
    /// The key as gpui names it (`"down"`, `"pagedown"`…).
    pub fn from_key(key: &str) -> Option<RowKey> {
        Some(match key {
            "down" => RowKey::Down,
            "up" => RowKey::Up,
            "pagedown" => RowKey::PageDown,
            "pageup" => RowKey::PageUp,
            "home" => RowKey::Home,
            "end" => RowKey::End,
            _ => return None,
        })
    }
}

/// The row `key` lands on in a view of `len` rows with `in_sight` rows in
/// sight from `top`. With no selected row, ↓ starts at the first row in
/// sight and ↑ at the last one; a page is the rows in sight.
pub fn row_for_key(key: RowKey, selected: Option<usize>, top: usize, in_sight: usize, len: usize) -> Option<usize> {
    let last = len.checked_sub(1)?;
    let page = in_sight.max(1);
    let base = selected.unwrap_or(top).min(last);
    Some(match (key, selected) {
        (RowKey::Down, None) => top.min(last),
        (RowKey::Up, None) => (top + page - 1).min(last),
        (RowKey::Down, Some(_)) => (base + 1).min(last),
        (RowKey::Up, Some(_)) => base.saturating_sub(1),
        (RowKey::PageDown, _) => (base + page).min(last),
        (RowKey::PageUp, _) => base.saturating_sub(page),
        (RowKey::Home, _) => 0,
        (RowKey::End, _) => last,
    })
}

/// Read a typed time on the files' clock: any form a log line has
/// (`2026-10-20 10:05:00`, `20/Oct/2026:10:05:00`), or a time of day alone
/// (`10:05`, `10:05:30`) on the day of `day_of`.
pub fn parse_time(text: &str, day_of: Option<f64>) -> Option<f64> {
    let text = text.trim();
    if let Some(t) = crate::parser::text::read_timestamp(text) {
        return Some(t);
    }
    let mut parts = text.split(':');
    let (h, m) = (parts.next()?, parts.next()?);
    let s = parts.next().unwrap_or("0");
    if parts.next().is_some() || h.is_empty() || h.len() > 2 || m.len() != 2 {
        return None;
    }
    let (h, m, s): (u32, u32, f64) = (h.parse().ok()?, m.parse().ok()?, s.replace(',', ".").parse().ok()?);
    if h > 23 || m > 59 || !(0.0..60.0).contains(&s) {
        return None;
    }
    let day = (day_of? / 86_400.0).floor() * 86_400.0;
    Some(day + f64::from(h * 3600 + m * 60) + s)
}

/// Split `name:line` typed into Go to Line, when `name` is one of `names`
/// (case aside): the name and the line text.
pub fn split_file_line<'a>(text: &'a str, names: &[&str]) -> Option<(usize, &'a str)> {
    let (name, line) = text.trim().rsplit_once(':')?;
    let ix = names.iter().position(|n| n.eq_ignore_ascii_case(name.trim()))?;
    Some((ix, line))
}

#[cfg(test)]
mod time_tests {
    use super::*;

    #[test]
    fn reads_times_of_day_on_the_day_given() {
        let day = parse_time("2026-10-20 00:00:00", None).unwrap();
        assert_eq!(parse_time("10:05", Some(day + 50.0)), Some(day + 36_300.0));
        assert_eq!(parse_time("10:05:30", Some(day)), Some(day + 36_330.0));
        assert_eq!(parse_time("10:05", None), None, "a time of day needs a day");
        for text in ["25:00", "10:5", "1:2:3:4", "abc", "12"] {
            assert_eq!(parse_time(text, Some(day)), None, "{text}");
        }
    }

    #[test]
    fn splits_a_file_name_off() {
        assert_eq!(split_file_line("B.log:5", &["a.log", "b.log"]), Some((1, "5")));
        assert_eq!(split_file_line("10:05", &["a.log"]), None);
    }
}
