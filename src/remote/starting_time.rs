//! Starting Time: where reading a Remote File begins, as a time on the clock
//! of the file itself. A span such as `1h` is counted back from the newest
//! line of the file, never from the clock of this machine or the Server, so a
//! log written in UTC on a Server in another zone still gives its last hour.

use crate::filter::parse_time_bound;

const DAY: f64 = 86_400.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// The text is neither a span nor a time.
    Invalid,
    /// A span or a time of day needs a time in the file, and it has none.
    NoTimestamps,
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::Invalid => write!(f, "not a time or a span like 30m, 2h30m, 2d"),
            StartError::NoTimestamps => write!(f, "no timestamps found"),
        }
    }
}

/// A span written as `30m`, `1h`, `2h30m`, `2d`, `1d12h`, `90s`: seconds.
pub fn parse_span(text: &str) -> Option<f64> {
    let t = text.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let mut total = 0.0;
    let mut digits = String::new();
    // Units must come in order, each at most once.
    let mut last_rank = 0;
    for c in t.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let (rank, unit) = match c {
            'd' => (1, DAY),
            'h' => (2, 3600.0),
            'm' => (3, 60.0),
            's' => (4, 1.0),
            _ => return None,
        };
        if digits.is_empty() || rank <= last_rank {
            return None;
        }
        total += digits.parse::<f64>().ok()? * unit;
        digits.clear();
        last_rank = rank;
    }
    (digits.is_empty() && total > 0.0).then_some(total)
}

/// The Starting Time `text` means, given `newest`: the time of the newest
/// line in the end of the file (`None` when it has no times). Empty text is
/// `Ok(None)`: read as usual.
pub fn resolve_start(text: &str, newest: Option<f64>) -> Result<Option<f64>, StartError> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if let Some(span) = parse_span(t) {
        return newest.map(|n| Some(n - span)).ok_or(StartError::NoTimestamps);
    }
    // A time of day reads differently on different days; one with a date does not.
    let on_day_0 = parse_time_bound(t, 0.0).map_err(|_| StartError::Invalid)?;
    let on_day_1 = parse_time_bound(t, 1e9).map_err(|_| StartError::Invalid)?;
    let Some(first) = on_day_0 else { return Ok(None) };
    if on_day_1 == on_day_0 {
        return Ok(Some(first));
    }
    let newest = newest.ok_or(StartError::NoTimestamps)?;
    let day = (newest / DAY).floor() * DAY;
    let at = day + first;
    Ok(Some(if at > newest { at - DAY } else { at }))
}

/// Whether `text` is readable as a Starting Time at all, for the field to
/// show an error while typing (a time of day is read as if a time existed).
pub fn is_readable(text: &str) -> bool {
    resolve_start(text, Some(DAY * 20_000.0)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::line_time::line_timestamp;

    fn at(text: &str) -> f64 {
        line_timestamp(&format!("{text} INFO x")).unwrap()
    }

    #[test]
    fn spans() {
        assert_eq!(parse_span("30m"), Some(1800.0));
        assert_eq!(parse_span("1h"), Some(3600.0));
        assert_eq!(parse_span("2h30m"), Some(9000.0));
        assert_eq!(parse_span("2d"), Some(172_800.0));
        assert_eq!(parse_span("1D12H"), Some(129_600.0));
        for bad in ["", "m", "30", "1h2d", "1h1h", "một giờ", "0m", "10:00"] {
            assert_eq!(parse_span(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_span_counts_back_from_the_newest_line() {
        let newest = at("2026-10-19 10:00:00");
        assert_eq!(resolve_start("1h", Some(newest)), Ok(Some(at("2026-10-19 09:00:00"))));
        assert_eq!(resolve_start("2d", Some(newest)), Ok(Some(at("2026-10-17 10:00:00"))));
    }

    #[test]
    fn an_absolute_time_does_not_need_the_newest_line() {
        let t = resolve_start("2026-10-19 10:00", None).unwrap().unwrap();
        assert_eq!(t, at("2026-10-19 10:00:00"));
    }

    #[test]
    fn a_time_of_day_takes_the_day_of_the_newest_line() {
        let newest = at("2026-10-19 11:30:00");
        assert_eq!(resolve_start("10:42", Some(newest)), Ok(Some(at("2026-10-19 10:42:00"))));
    }

    #[test]
    fn a_time_of_day_after_the_newest_line_goes_back_a_day() {
        let newest = at("2026-10-19 00:20:00");
        assert_eq!(resolve_start("23:30", Some(newest)), Ok(Some(at("2026-10-18 23:30:00"))));
    }

    #[test]
    fn no_timestamps_in_the_file() {
        assert_eq!(resolve_start("1h", None), Err(StartError::NoTimestamps));
        assert_eq!(resolve_start("10:42", None), Err(StartError::NoTimestamps));
    }

    #[test]
    fn empty_and_unreadable() {
        assert_eq!(resolve_start("  ", None), Ok(None));
        assert_eq!(resolve_start("một giờ", Some(1e9)), Err(StartError::Invalid));
        assert!(is_readable("30m") && is_readable("10:42") && !is_readable("abc"));
    }
}
