//! Resume By Time: running an unedited Command Preset again from the time of
//! the last line already received, and dropping the lines at that boundary
//! that were already received.
//!
//! The time is the one the Server wrote in the output (with its own offset):
//! never the local clock, never converted to another Server's time (L8).

use crate::model::Stream;
use crate::remote::command::{PresetFields, Start};
use std::collections::VecDeque;

/// The boundary keeps at most this many lines; more and resuming is given up.
pub const MAX_BOUNDARY_LINES: usize = 10_000;
/// Lines older than the mark dropped, without a newer line yet, before the
/// Server's clock is taken to have gone back.
pub const CLOCK_BACK_LINES: usize = 1_000;

/// A time read from the first token of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timestamp {
    /// Nanoseconds since the epoch.
    pub nanos: i128,
    /// The token exactly as the Server wrote it.
    pub text: String,
    /// Seconds into the day in the Server's own offset.
    pub seconds_of_day: u32,
}

impl Timestamp {
    fn epoch_seconds(&self) -> i128 {
        self.nanos.div_euclid(1_000_000_000)
    }
}

/// How precisely the since flag of a tool takes a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Precision {
    Second,
    Nano,
}

fn precision(preset: &PresetFields) -> Precision {
    match preset {
        PresetFields::Journal { .. } => Precision::Second,
        PresetFields::Docker { .. } | PresetFields::Kube { .. } => Precision::Nano,
    }
}

/// The time at the start of a line of this preset's output, if it has one:
/// `2026-10-19T10:02:03.123456+0700` (journalctl `short-iso-precise`) or
/// `2026-10-19T03:02:03.123456789Z` (Docker and kubectl `--timestamps`).
pub fn parse_timestamp(_preset: &PresetFields, line: &str) -> Option<Timestamp> {
    let token = line.split_whitespace().next()?;
    parse_rfc3339(token)
}

fn parse_rfc3339(token: &str) -> Option<Timestamp> {
    let b = token.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b't') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |from: usize, len: usize| -> Option<i64> {
        let part = token.get(from..from + len)?;
        part.bytes().all(|c| c.is_ascii_digit()).then(|| part.parse().ok())?
    };
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = &token[19..];
    let mut nanos: i128 = 0;
    if let Some(after) = rest.strip_prefix('.') {
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let used = &after[..digits.min(9)];
        nanos = used.parse::<i128>().ok()? * 10i128.pow(9 - used.len() as u32);
        rest = &after[digits..];
    }
    // The offset is required: without it the time is not the Server's.
    let offset_seconds: i64 = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits: String = rest[1..].chars().filter(|c| *c != ':').collect();
            if digits.len() != 4 || !digits.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let (oh, om): (i64, i64) = (digits[..2].parse().ok()?, digits[2..].parse().ok()?);
            sign * (oh * 3600 + om * 60)
        }
    };
    let days = days_from_civil(year, month, day);
    let local = days * 86_400 + hour * 3600 + minute * 60 + second;
    let epoch = local - offset_seconds;
    Some(Timestamp {
        nanos: i128::from(epoch) * 1_000_000_000 + nanos,
        text: token.to_string(),
        seconds_of_day: (hour * 3600 + minute * 60 + second) as u32,
    })
}

/// Days from 1970-01-01 to a date of the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// One line kept for the boundary.
#[derive(Debug, Clone)]
struct Kept {
    /// The time of the line, or of the line before it for a continuation.
    key: i128,
    text: String,
}

/// What a resumed Run does with a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Keep,
    /// Already received.
    Drop,
    /// Kept, and the Server's clock seems to have gone back: lines may repeat.
    KeepClockWentBack,
}

/// What the filter of a resumed Run knows.
#[derive(Debug, Clone)]
struct Filtering {
    /// Lines already received, per stream, still to be matched.
    boundary: [Vec<String>; 2],
    floor: i128,
    last: i128,
    /// Lines dropped for being older than `floor`.
    older_dropped: usize,
}

/// The time of the last timestamped line received and the lines at that
/// boundary, kept while a Remote Command from an unedited Command Preset
/// runs.
#[derive(Debug, Clone)]
pub struct ResumePoint {
    preset: PresetFields,
    last: Option<Timestamp>,
    /// Lines received from the second (or nanosecond) of `last` on, per stream.
    kept: [VecDeque<Kept>; 2],
    /// The key a line without a time gets (the time of the line before it).
    carry: [i128; 2],
    overflowed: bool,
    filtering: Option<Filtering>,
}

fn slot(stream: Stream) -> usize {
    match stream {
        Stream::Stdout => 0,
        Stream::Stderr => 1,
    }
}

impl ResumePoint {
    pub fn new(preset: PresetFields) -> Self {
        Self {
            preset,
            last: None,
            kept: [VecDeque::new(), VecDeque::new()],
            carry: [i128::MIN; 2],
            overflowed: false,
            filtering: None,
        }
    }

    pub fn preset(&self) -> &PresetFields {
        &self.preset
    }

    fn floor_of(&self, ts: &Timestamp) -> i128 {
        match precision(&self.preset) {
            Precision::Second => ts.epoch_seconds() * 1_000_000_000,
            Precision::Nano => ts.nanos,
        }
    }

    /// A line that was kept and shown.
    pub fn record(&mut self, stream: Stream, line: &str) {
        if self.overflowed {
            return;
        }
        let i = slot(stream);
        let key = match parse_timestamp(&self.preset, line) {
            Some(ts) => {
                self.carry[i] = ts.nanos;
                let key = ts.nanos;
                self.last = Some(ts);
                key
            }
            None => self.carry[i],
        };
        self.kept[i].push_back(Kept { key, text: line.to_string() });
        if let Some(last) = self.last.clone() {
            let floor = self.floor_of(&last);
            for queue in &mut self.kept {
                while queue.front().is_some_and(|k| k.key < floor) {
                    queue.pop_front();
                }
            }
        }
        if self.kept.iter().map(VecDeque::len).sum::<usize>() > MAX_BOUNDARY_LINES {
            self.overflowed = true;
            self.kept = [VecDeque::new(), VecDeque::new()];
        }
    }

    /// Whether the command can be run again from the last line: it has had a
    /// line with a time and the boundary did not overflow.
    pub fn can_resume(&self) -> bool {
        self.last.is_some() && !self.overflowed
    }

    /// The time of the last line, as shown in a Break (`HH:MM:SS` seconds).
    pub fn last_seconds_of_day(&self) -> Option<u32> {
        self.last.as_ref().map(|t| t.seconds_of_day)
    }

    /// The command to run again from the last line.
    pub fn command(&self) -> Option<String> {
        if !self.can_resume() {
            return None;
        }
        let last = self.last.as_ref()?;
        let since = match precision(&self.preset) {
            Precision::Second => format!("@{}", last.epoch_seconds()),
            Precision::Nano => last.text.clone(),
        };
        Some(self.preset.render(&Start::Since(since)))
    }

    /// Start filtering the lines of the Run about to begin; call just before
    /// running `command()`.
    pub fn begin_resume(&mut self) {
        let Some(last) = self.last.clone() else { return };
        let floor = self.floor_of(&last);
        let boundary = [0, 1].map(|i| self.kept[i].iter().map(|k| k.text.clone()).collect());
        self.filtering = Some(Filtering { boundary, floor, last: last.nanos, older_dropped: 0 });
    }

    /// Whether the lines of the Run in progress are still being compared with
    /// the boundary.
    pub fn is_filtering(&self) -> bool {
        self.filtering.is_some()
    }

    /// What to do with a line of a resumed Run. Lines that are kept must also
    /// be given to `record`.
    pub fn filter(&mut self, stream: Stream, line: &str) -> Verdict {
        let Some(state) = self.filtering.as_mut() else { return Verdict::Keep };
        let ts = parse_timestamp(&self.preset, line);
        // A line newer than the mark ends the comparison.
        if let Some(ts) = &ts {
            if ts.nanos > state.last {
                self.filtering = None;
                return Verdict::Keep;
            }
            if ts.nanos < state.floor {
                state.older_dropped += 1;
                if state.older_dropped > CLOCK_BACK_LINES {
                    self.filtering = None;
                    return Verdict::KeepClockWentBack;
                }
                return Verdict::Drop;
            }
        }
        let boundary = &mut state.boundary[slot(stream)];
        match boundary.iter().position(|b| b == line) {
            Some(at) => {
                boundary.remove(at);
                Verdict::Drop
            }
            None => {
                self.filtering = None;
                Verdict::Keep
            }
        }
    }

    /// The Run ended: no more comparing for the next one unless asked.
    pub fn end_filtering(&mut self) {
        self.filtering = None;
    }
}
