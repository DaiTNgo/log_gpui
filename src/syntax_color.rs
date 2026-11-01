//! Syntax Color: the tokens of a log line, found in one pass, for painting each
//! kind in its own color. Pure text in, byte ranges out; the painting lives in
//! `ui::canvas`.

use crate::model::LogLevel;
use std::ops::Range;

/// Longest prefix of a line that is tokenized; the rest keeps the default
/// color.
pub const MAX_TOKENIZE_BYTES: usize = 4096;

/// How far into the line (after the timestamp) a level keyword is looked for.
const LEVEL_WINDOW: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Timestamp,
    /// A level keyword, painted in the color of its level.
    Level(LogLevel),
    /// A stack trace line (a Continuation Line).
    StackTrace,
    /// A `Caused by:` line.
    CausedBy,
    /// The logger or class name after the level (`com.foo.Bar`).
    Module,
    /// A number, with its unit if any (`42`, `12ms`, `1.5s`, `80%`).
    Number,
    /// A string in double quotes, quotes included.
    String,
    /// A UUID, an IPv4 address (with a port) or an `http(s)` URL.
    Identifier,
    /// The key of a `key=value` pair.
    Key,
}

/// A token: a range of bytes of the stored line, on char boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub range: Range<usize>,
    pub kind: TokenKind,
}

/// What text is being tokenized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// A whole stored line (the Raw View, the Raw tab of the Focus Pane): a
    /// Continuation Line is recognized by its shape.
    Line,
    /// The message of an entry whose timestamp, level and module have their
    /// own cells; `continuation` is what the parser says.
    Message { continuation: bool },
}

/// The tokens of `line`, in order and without overlap.
pub fn tokenize(line: &str, scope: Scope) -> Vec<Token> {
    let end = floor_boundary(line, line.len().min(MAX_TOKENIZE_BYTES));
    let head = &line[..end];
    let bytes = head.as_bytes();
    let mut tokens = Vec::new();

    let continuation = match scope {
        Scope::Message { continuation } => continuation,
        Scope::Line => looks_like_continuation(bytes),
    };
    if continuation {
        let trimmed = head.trim_start();
        let kind = if trimmed.starts_with("Caused by:") { TokenKind::CausedBy } else { TokenKind::StackTrace };
        if !head.is_empty() {
            tokens.push(Token { range: 0..end, kind });
        }
        return tokens;
    }
    let mut cursor = 0;
    let mut specials: Vec<Token> = Vec::new();
    if scope == Scope::Line {
        if let Some(range) = timestamp_at(bytes) {
            cursor = range.end;
            tokens.push(Token { range, kind: TokenKind::Timestamp });
        }
        if let Some((range, level)) = level_after(bytes, cursor) {
            let after = range.end;
            specials.push(Token { range, kind: TokenKind::Level(level) });
            if let Some(range) = module_after(bytes, after) {
                specials.push(Token { range, kind: TokenKind::Module });
            }
        }
    }
    scan_rest(bytes, cursor, &specials, &mut tokens);
    tokens
}

/// A line is a Continuation Line by shape when it is indented or opens with
/// `Caused by:`, and does not start with a timestamp.
fn looks_like_continuation(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if bytes.starts_with(b"Caused by:") {
        return true;
    }
    matches!(bytes[0], b' ' | b'\t') && bytes.iter().any(|b| !b.is_ascii_whitespace())
}

fn digits(bytes: &[u8], from: usize, n: usize) -> bool {
    bytes.len() >= from + n && bytes[from..from + n].iter().all(u8::is_ascii_digit)
}

/// The timestamp at the start of the line (after an opening `[`): ISO date
/// and time, or a time of day.
fn timestamp_at(bytes: &[u8]) -> Option<Range<usize>> {
    let start = usize::from(bytes.first() == Some(&b'['));
    let mut i = start;
    if digits(bytes, i, 4) && bytes.get(i + 4) == Some(&b'-') && digits(bytes, i + 5, 2) && bytes.get(i + 7) == Some(&b'-') && digits(bytes, i + 8, 2) {
        i += 10;
        if matches!(bytes.get(i), Some(b'T' | b' ')) && digits(bytes, i + 1, 2) && bytes.get(i + 3) == Some(&b':') {
            i += 1;
            i = time_of_day(bytes, i)?;
            i = fraction_and_zone(bytes, i);
        }
        return Some(start..i);
    }
    if digits(bytes, i, 2) && bytes.get(i + 2) == Some(&b':') {
        let end = time_of_day(bytes, i)?;
        return Some(start..fraction_and_zone(bytes, end));
    }
    None
}

/// `HH:MM` or `HH:MM:SS` at `i`; the index after it.
fn time_of_day(bytes: &[u8], i: usize) -> Option<usize> {
    if !(digits(bytes, i, 2) && bytes.get(i + 2) == Some(&b':') && digits(bytes, i + 3, 2)) {
        return None;
    }
    if bytes.get(i + 5) == Some(&b':') && digits(bytes, i + 6, 2) {
        Some(i + 8)
    } else {
        Some(i + 5)
    }
}

/// An optional `.123` or `,123`, then an optional `Z` or `+02:00`.
fn fraction_and_zone(bytes: &[u8], mut i: usize) -> usize {
    if matches!(bytes.get(i), Some(b'.' | b',')) && digits(bytes, i + 1, 1) {
        i += 1;
        while digits(bytes, i, 1) {
            i += 1;
        }
    }
    match bytes.get(i) {
        Some(b'Z') => i + 1,
        Some(b'+' | b'-') if digits(bytes, i + 1, 2) => {
            let mut j = i + 3;
            if bytes.get(j) == Some(&b':') {
                j += 1;
            }
            if digits(bytes, j, 2) { j + 2 } else { i }
        }
        _ => i,
    }
}

const LEVELS: [(&str, LogLevel); 8] = [
    ("FATAL", LogLevel::Fatal),
    ("ERROR", LogLevel::Error),
    ("WARNING", LogLevel::Warn),
    ("WARN", LogLevel::Warn),
    ("INFO", LogLevel::Info),
    ("DEBUG", LogLevel::Debug),
    ("TRACE", LogLevel::Verbose),
    ("VERBOSE", LogLevel::Verbose),
];

/// The first level keyword, in capitals and standing alone, within
/// `LEVEL_WINDOW` bytes after `from`.
fn level_after(bytes: &[u8], from: usize) -> Option<(Range<usize>, LogLevel)> {
    let stop = bytes.len().min(from + LEVEL_WINDOW);
    let mut i = from;
    while i < stop {
        if bytes[i].is_ascii_uppercase() && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()) {
            for (word, level) in LEVELS {
                let end = i + word.len();
                if bytes[i..].starts_with(word.as_bytes()) && bytes.get(end).is_none_or(|b| !b.is_ascii_alphanumeric()) {
                    return Some((i..end, level));
                }
            }
        }
        i += 1;
    }
    None
}

/// Bytes that are part of a word; bytes of multi-byte chars count, so a word
/// is never cut inside one.
fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// The logger or class name that opens the message after a level: a dotted
/// name (`com.foo.Bar`), after any `[thread]` groups and separators.
fn module_after(bytes: &[u8], from: usize) -> Option<Range<usize>> {
    let stop = bytes.len().min(from + 2 * LEVEL_WINDOW);
    let mut i = from;
    while i < stop {
        match bytes[i] {
            b' ' | b']' | b'-' | b':' | b'|' => i += 1,
            b'[' => i += bytes[i..stop].iter().position(|&b| b == b']')? + 1,
            _ => break,
        }
    }
    let start = i;
    let ident = |mut j: usize| {
        if !(bytes.get(j).is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_' || *b == b'$')) {
            return None;
        }
        while bytes.get(j).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$') {
            j += 1;
        }
        Some(j)
    };
    let mut j = ident(start)?;
    let mut dots = 0;
    while bytes.get(j) == Some(&b'.') {
        match ident(j + 1) {
            Some(next) => {
                j = next;
                dots += 1;
            }
            None => break,
        }
    }
    (dots > 0 && j <= stop && bytes.get(j).is_none_or(|b| !is_word(*b))).then_some(start..j)
}

enum Scan {
    Token(usize),
    Skip(usize),
}

const UNITS: [&str; 14] = ["KiB", "MiB", "GiB", "KB", "MB", "GB", "ms", "us", "ns", "s", "m", "h", "d", "B"];

fn is_digit_at(bytes: &[u8], i: usize) -> bool {
    bytes.get(i).is_some_and(u8::is_ascii_digit)
}

fn digit_run(bytes: &[u8], mut i: usize) -> usize {
    while is_digit_at(bytes, i) {
        i += 1;
    }
    i
}

/// A number at `i` (a digit that starts a word): `Token` with the end of the
/// number and its unit, or `Skip` over something that only looks like one
/// (a date, a version, `123abc`).
fn number_at(bytes: &[u8], i: usize) -> Scan {
    let mut j = digit_run(bytes, i);
    if bytes.get(j) == Some(&b'.') && is_digit_at(bytes, j + 1) {
        j = digit_run(bytes, j + 1);
    }
    let separated = matches!(bytes.get(j), Some(b'.' | b'-' | b':' | b'/')) && is_digit_at(bytes, j + 1);
    if separated {
        let mut k = j;
        while bytes.get(k).is_some_and(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b':' | b'/')) {
            k += 1;
        }
        return Scan::Skip(k);
    }
    if bytes.get(j) == Some(&b'%') {
        return Scan::Token(j + 1);
    }
    if bytes.get(j).is_some_and(|b| is_word(*b)) {
        for unit in UNITS {
            let end = j + unit.len();
            if bytes[j..].starts_with(unit.as_bytes()) && bytes.get(end).is_none_or(|b| !is_word(*b)) {
                return Scan::Token(end);
            }
        }
        let mut k = j;
        while bytes.get(k).is_some_and(|b| is_word(*b)) {
            k += 1;
        }
        return Scan::Skip(k);
    }
    Scan::Token(j)
}

fn uuid_at(bytes: &[u8], i: usize) -> Option<usize> {
    let end = i + 36;
    if bytes.len() < end {
        return None;
    }
    let ok = bytes[i..end].iter().enumerate().all(|(n, b)| match n {
        8 | 13 | 18 | 23 => *b == b'-',
        _ => b.is_ascii_hexdigit(),
    });
    (ok && bytes.get(end).is_none_or(|b| !is_word(*b))).then_some(end)
}

/// An IPv4 address at `i`, with an optional `:port`.
fn ipv4_at(bytes: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    for octet in 0..4 {
        let end = digit_run(bytes, j);
        if end == j || end - j > 3 || std::str::from_utf8(&bytes[j..end]).ok()?.parse::<u16>().ok()? > 255 {
            return None;
        }
        j = end;
        if octet < 3 {
            if bytes.get(j) != Some(&b'.') {
                return None;
            }
            j += 1;
        }
    }
    if bytes.get(j) == Some(&b':') && is_digit_at(bytes, j + 1) {
        let end = digit_run(bytes, j + 1);
        if end - j - 1 <= 5 {
            j = end;
        }
    }
    let more = bytes.get(j) == Some(&b'.') && is_digit_at(bytes, j + 1);
    (!more && bytes.get(j).is_none_or(|b| !is_word(*b))).then_some(j)
}

/// The end of the string that opens with the quote at `i`, quotes included,
/// if it closes before `limit`.
fn string_end(bytes: &[u8], i: usize, limit: usize) -> Option<usize> {
    let mut from = i + 1;
    loop {
        let close = from + memchr::memchr(b'"', &bytes[from..limit])?;
        let backslashes = bytes[i + 1..close].iter().rev().take_while(|b| **b == b'\\').count();
        if backslashes % 2 == 0 {
            return Some(close + 1);
        }
        from = close + 1;
    }
}

/// The end of the `http(s)` URL at `i`, without trailing punctuation.
fn url_at(bytes: &[u8], i: usize, limit: usize) -> Option<usize> {
    let rest = &bytes[i..limit];
    let scheme = if rest.starts_with(b"https://") {
        8
    } else if rest.starts_with(b"http://") {
        7
    } else {
        return None;
    };
    let mut end = i + scheme;
    while end < limit && !bytes[end].is_ascii_whitespace() && !matches!(bytes[end], b'"' | b'\'' | b'<' | b'>' | b')' | b']') {
        end += 1;
    }
    while end > i + scheme && matches!(bytes[end - 1], b'.' | b',' | b';' | b':') {
        end -= 1;
    }
    (end > i + scheme).then_some(end)
}

/// The key of a `key=value` pair that starts at `i`: the end of the key when
/// `=` and a value follow.
fn key_at(bytes: &[u8], i: usize, limit: usize) -> Option<usize> {
    if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return None;
    }
    let mut j = i;
    while j < limit {
        let b = bytes[j];
        let joins = matches!(b, b'.' | b'-') && bytes.get(j + 1).is_some_and(|n| n.is_ascii_alphanumeric());
        if b.is_ascii_alphanumeric() || b == b'_' || joins {
            j += 1;
        } else {
            break;
        }
    }
    let value = bytes.get(j + 1)?;
    (j < limit && bytes[j] == b'=' && !value.is_ascii_whitespace() && *value != b'=').then_some(j)
}

/// Tokens of the rest of the line from `from`, with the `specials` (level,
/// module) put in at their places.
fn scan_rest(bytes: &[u8], from: usize, specials: &[Token], out: &mut Vec<Token>) {
    let len = bytes.len();
    let mut i = from;
    let mut k = 0;
    while i < len {
        while specials.get(k).is_some_and(|s| s.range.end <= i) {
            k += 1;
        }
        if let Some(sp) = specials.get(k) {
            if sp.range.start == i {
                out.push(sp.clone());
                i = sp.range.end;
                continue;
            }
        }
        let limit = specials.get(k).map_or(len, |s| s.range.start);
        let b = bytes[i];
        let at_boundary = i == 0 || !is_word(bytes[i - 1]);
        let push = |end: usize, kind: TokenKind, out: &mut Vec<Token>| {
            out.push(Token { range: i..end, kind });
            end
        };
        if b == b'"' {
            i = match string_end(bytes, i, limit) {
                Some(end) => push(end, TokenKind::String, out),
                None => i + 1,
            };
        } else if b.is_ascii_digit() {
            let after_dot = i > 0 && bytes[i - 1] == b'.';
            if !at_boundary || after_dot {
                i = digit_run(bytes, i);
            } else if let Some(end) = uuid_at(bytes, i).or_else(|| ipv4_at(bytes, i)).filter(|e| *e <= limit) {
                i = push(end, TokenKind::Identifier, out);
            } else {
                match number_at(bytes, i) {
                    Scan::Token(end) if end <= limit => i = push(end, TokenKind::Number, out),
                    Scan::Token(end) | Scan::Skip(end) => i = end.min(limit).max(i + 1),
                }
            }
        } else if is_word(b) && at_boundary {
            if let Some(end) = uuid_at(bytes, i).filter(|e| *e <= limit) {
                i = push(end, TokenKind::Identifier, out);
            } else if let Some(end) = url_at(bytes, i, limit).filter(|_| b == b'h') {
                i = push(end, TokenKind::Identifier, out);
            } else if let Some(end) = key_at(bytes, i, limit) {
                push(end, TokenKind::Key, out);
                i = end + 1;
            } else {
                while i < len && is_word(bytes[i]) {
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }
}

fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str, scope: Scope) -> Vec<(&str, TokenKind)> {
        tokenize(line, scope).into_iter().map(|t| (&line[t.range], t.kind)).collect()
    }

    #[test]
    fn test_timestamp_and_level() {
        assert_eq!(
            kinds("2024-05-01 10:22:03.512 ERROR payment failed", Scope::Line),
            vec![("2024-05-01 10:22:03.512", TokenKind::Timestamp), ("ERROR", TokenKind::Level(LogLevel::Error))]
        );
    }

    #[test]
    fn test_iso_timestamp_with_zone_in_brackets() {
        assert_eq!(
            kinds("[2024-05-01T10:22:03,512+02:00] [WARN] slow", Scope::Line),
            vec![("2024-05-01T10:22:03,512+02:00", TokenKind::Timestamp), ("WARN", TokenKind::Level(LogLevel::Warn))]
        );
    }

    #[test]
    fn test_time_of_day_only() {
        assert_eq!(
            kinds("10:22:03 INFO started", Scope::Line),
            vec![("10:22:03", TokenKind::Timestamp), ("INFO", TokenKind::Level(LogLevel::Info))]
        );
    }

    #[test]
    fn test_lowercase_level_is_not_a_level() {
        assert_eq!(kinds("something error happened", Scope::Line), vec![]);
        assert_eq!(kinds("the ERRORS list", Scope::Line), vec![]);
    }

    #[test]
    fn test_level_far_into_the_line_is_not_a_level() {
        let line = format!("{} ERROR", "x ".repeat(60));
        assert_eq!(kinds(&line, Scope::Line), vec![]);
    }

    #[test]
    fn test_stack_trace_and_caused_by() {
        assert_eq!(
            kinds("\tat com.foo.Bar.run(Bar.java:42)", Scope::Line),
            vec![("\tat com.foo.Bar.run(Bar.java:42)", TokenKind::StackTrace)]
        );
        assert_eq!(
            kinds("Caused by: java.io.IOException", Scope::Line),
            vec![("Caused by: java.io.IOException", TokenKind::CausedBy)]
        );
        assert_eq!(
            kinds("   at foo", Scope::Message { continuation: true }),
            vec![("   at foo", TokenKind::StackTrace)]
        );
    }

    #[test]
    fn test_message_scope_has_no_timestamp_or_level() {
        assert_eq!(kinds("2024-05-01 10:22:03 ERROR x", Scope::Message { continuation: false }), vec![]);
    }

    #[test]
    fn test_long_line_is_cut_on_a_char_boundary() {
        let line = format!("10:22:03 INFO {}", "đ".repeat(MAX_TOKENIZE_BYTES));
        let tokens = tokenize(&line, Scope::Line);
        assert_eq!(tokens.len(), 2);
        let line = "\t".to_string() + &"đ".repeat(MAX_TOKENIZE_BYTES);
        let tokens = tokenize(&line, Scope::Line);
        assert!(line.is_char_boundary(tokens[0].range.end));
        assert!(tokens[0].range.end <= MAX_TOKENIZE_BYTES);
    }

    #[test]
    fn test_vietnamese_and_emoji_line() {
        assert_eq!(
            kinds("10:22:03 INFO đã thanh toán 🎉", Scope::Line),
            vec![("10:22:03", TokenKind::Timestamp), ("INFO", TokenKind::Level(LogLevel::Info))]
        );
    }

    const MSG: Scope = Scope::Message { continuation: false };

    #[test]
    fn test_key_value() {
        assert_eq!(
            kinds("user=alice took=12ms", MSG),
            vec![("user", TokenKind::Key), ("took", TokenKind::Key), ("12ms", TokenKind::Number)]
        );
        assert_eq!(kinds("a = b", MSG), vec![]);
        assert_eq!(kinds("a==b", MSG), vec![]);
    }

    #[test]
    fn test_numbers_and_units() {
        assert_eq!(
            kinds("took 42 then 1.5s, 80% and 3 MB 512MB", MSG),
            vec![
                ("42", TokenKind::Number),
                ("1.5s", TokenKind::Number),
                ("80%", TokenKind::Number),
                ("3", TokenKind::Number),
                ("512MB", TokenKind::Number)
            ]
        );
    }

    #[test]
    fn test_number_inside_words_is_not_a_number() {
        assert_eq!(kinds("abc123 v2 2fa 123abc 0x1f", MSG), vec![]);
        assert_eq!(kinds("version 1.2.3 on 2024-05-01 at 10:22:03", MSG), vec![]);
    }

    #[test]
    fn test_strings() {
        assert_eq!(
            kinds(r#"said "hello \"you\" there" ok"#, MSG),
            vec![(r#""hello \"you\" there""#, TokenKind::String)]
        );
        assert_eq!(kinds(r#"unclosed "quote 5"#, MSG), vec![("5", TokenKind::Number)]);
    }

    #[test]
    fn test_uuid_ip_and_url() {
        assert_eq!(
            kinds("req 3f2b8c1e-6a4d-4e0b-9a52-1c7d0e5f8a90 from 10.0.0.12:8080 see https://x.io/a?b=1.", MSG),
            vec![
                ("3f2b8c1e-6a4d-4e0b-9a52-1c7d0e5f8a90", TokenKind::Identifier),
                ("10.0.0.12:8080", TokenKind::Identifier),
                ("https://x.io/a?b=1", TokenKind::Identifier)
            ]
        );
        assert_eq!(kinds("300.1.1.1 1.2.3", MSG), vec![]);
    }

    #[test]
    fn test_module_after_level() {
        assert_eq!(
            kinds("2024-05-01 10:22:03 INFO [main] com.foo.Bar: started in 12ms", Scope::Line),
            vec![
                ("2024-05-01 10:22:03", TokenKind::Timestamp),
                ("INFO", TokenKind::Level(LogLevel::Info)),
                ("com.foo.Bar", TokenKind::Module),
                ("12ms", TokenKind::Number)
            ]
        );
        assert_eq!(kinds("INFO started now", Scope::Line), vec![("INFO", TokenKind::Level(LogLevel::Info))]);
    }

    #[test]
    fn test_tokens_are_in_order_without_overlap_on_char_boundaries() {
        let lines = [
            "2024-05-01 10:22:03.512 ERROR [t-1] org.a.B: đã \"thanh toán\" id=3f2b8c1e-6a4d-4e0b-9a52-1c7d0e5f8a90 🎉 5ms",
            "WARN \"ERROR\" ERROR",
            "ERROR a.b.c=1 \"x",
            "10:22:03 INFO 1.2.3.4:80 http://a.b/c)",
        ];
        for line in lines {
            let tokens = tokenize(line, Scope::Line);
            let mut last = 0;
            for t in &tokens {
                assert!(t.range.start >= last, "{line}: {tokens:?}");
                assert!(t.range.start < t.range.end);
                assert!(line.is_char_boundary(t.range.start) && line.is_char_boundary(t.range.end));
                last = t.range.end;
            }
        }
    }

    #[test]
    fn test_empty_line() {
        assert_eq!(kinds("", Scope::Line), vec![]);
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// Not a gate: prints the cost of tokenizing one screenful.
    #[test]
    #[ignore]
    fn tokenize_a_screenful() {
        let line = "2024-05-01 10:22:03.512 INFO [worker-3] com.acme.payment.Gateway: charged user=alice order=48213 took=12ms from 10.0.0.12:8080 id=3f2b8c1e-6a4d-4e0b-9a52-1c7d0e5f8a90 \"ok\"";
        let lines: Vec<String> = (0..100).map(|i| format!("{line} {i}")).collect();
        let start = std::time::Instant::now();
        let mut n = 0;
        for _ in 0..100 {
            for l in &lines {
                n += tokenize(l, Scope::Line).len();
            }
        }
        println!("100 lines x {} bytes: {:?} per screenful ({n} tokens / 100 runs)", line.len(), start.elapsed() / 100);
    }
}
