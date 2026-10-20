//! Merged timeline of a Log Space: the rows of every file interleaved by
//! timestamp (rule 22).
//!
//! Each file is parsed on its own and cached; the merged view is only a list
//! of `(file, line)` references ordered by the key
//! `(effective timestamp, file order, line)`. Changing the file order or the
//! set of files merges again without parsing anything.

use crate::model::LogEntry;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};
use std::sync::Arc;

/// A row of the merged timeline: an entry of one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowRef {
    /// Position of the file in the order handed to [`merge`].
    pub file: u32,
    /// Position of the entry in that file's entries.
    pub line: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MergedTimeline {
    pub rows: Vec<RowRef>,
    /// Timestamp each merged row sorts by. `NAN` for the rows of a file that
    /// has no timestamp at all.
    pub effective_ts: Vec<f64>,
    /// Positions (in the order handed to [`merge`]) of files with no
    /// timestamp at all; their rows come after every other row.
    pub no_timestamp_files: Vec<usize>,
}

impl MergedTimeline {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// A row has its own timestamp when it is a finite, positive number.
fn has_timestamp(entry: &LogEntry) -> bool {
    entry.has_time && entry.timestamp.is_finite() && entry.timestamp > 0.0
}

/// The timestamp each entry sorts by, or `None` when the file has no
/// timestamp at all. A continuation line takes the timestamp of the row above
/// it; lines before the first timestamp take that first timestamp.
pub fn effective_timestamps(entries: &[LogEntry]) -> Option<Vec<f64>> {
    let first = entries.iter().find(|e| has_timestamp(e))?.timestamp;
    let mut current = first;
    Some(
        entries
            .iter()
            .map(|e| {
                if has_timestamp(e) {
                    current = e.timestamp;
                }
                current
            })
            .collect(),
    )
}

/// Merge the entries of `files` (given in the Log Space's file order) by
/// timestamp. Timestamps are compared as written in each file: no timezone
/// conversion.
pub fn merge(files: &[&[LogEntry]]) -> MergedTimeline {
    struct Lane {
        ts: Vec<f64>,
        /// Whether each line continues the line above it.
        cont: Vec<bool>,
        next: usize,
    }

    let mut timeline = MergedTimeline::default();
    let mut lanes: Vec<Lane> = Vec::with_capacity(files.len());
    let mut tail: Vec<usize> = Vec::new();
    for (i, entries) in files.iter().enumerate() {
        match effective_timestamps(entries) {
            Some(ts) => lanes.push(Lane {
                ts,
                cont: entries.iter().map(|e| e.parent.is_some()).collect(),
                next: 0,
            }),
            None => {
                // Keeps the lane index aligned with the file index.
                lanes.push(Lane { ts: Vec::new(), cont: Vec::new(), next: 0 });
                if !entries.is_empty() {
                    timeline.no_timestamp_files.push(i);
                    tail.push(i);
                }
            }
        }
    }
    timeline.rows.reserve(files.iter().map(|f| f.len()).sum());
    timeline.effective_ts.reserve(timeline.rows.capacity());

    // Heap entries order by (timestamp bits, file, line). Timestamps are
    // finite and positive, so their bit patterns order like the numbers.
    let key = |ts: f64, file: usize, line: usize| Reverse((ts.to_bits(), file, line));
    let mut heap = BinaryHeap::new();
    for (file, lane) in lanes.iter().enumerate() {
        if let Some(&ts) = lane.ts.first() {
            heap.push(key(ts, file, 0));
        }
    }
    while let Some(Reverse((bits, file, line))) = heap.pop() {
        timeline.rows.push(RowRef { file: file as u32, line: line as u32 });
        timeline.effective_ts.push(f64::from_bits(bits));
        let lane = &mut lanes[file];
        lane.next = line + 1;
        // Continuation Lines stay right below their Parent Line, whatever
        // other file has a row at the same time.
        while lane.cont.get(lane.next).copied().unwrap_or(false) {
            timeline.rows.push(RowRef { file: file as u32, line: lane.next as u32 });
            timeline.effective_ts.push(f64::from_bits(bits));
            lane.next += 1;
        }
        if let Some(&ts) = lane.ts.get(lane.next) {
            heap.push(key(ts, file, lane.next));
        }
    }

    for file in tail {
        for line in 0..files[file].len() {
            timeline.rows.push(RowRef { file: file as u32, line: line as u32 });
            timeline.effective_ts.push(f64::NAN);
        }
    }
    timeline
}

/// Build the merged entries: each row is a copy of its file's entry tagged
/// with its Source File. `files` pairs each file's `file_id` and display name
/// with its entries, in the order handed to [`merge`].
pub fn materialize(
    files: &[(&str, &str, &[LogEntry])],
    timeline: &MergedTimeline,
) -> Vec<LogEntry> {
    let names: Vec<(Arc<str>, Arc<str>)> = files
        .iter()
        .map(|(id, name, _)| (Arc::from(*id), Arc::from(*name)))
        .collect();
    timeline
        .rows
        .iter()
        .map(|r| {
            let mut entry = files[r.file as usize].2[r.line as usize].clone();
            entry.file_id = Some(names[r.file as usize].0.clone());
            entry.source = Some(names[r.file as usize].1.clone());
            entry
        })
        .collect()
}

/// The columns of a Log Space: the union of the files' fields by name. The
/// first file's column order is kept; a column found only in a later file is
/// added after, in file order.
pub fn union_field_names(files: &[&[LogEntry]]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut names = Vec::new();
    for entries in files {
        let mut in_file = Vec::new();
        for entry in entries.iter() {
            for (name, _) in entry.fields() {
                if seen.insert(name.as_str()) {
                    in_file.push(name.as_str().to_string());
                }
            }
        }
        names.extend(in_file);
    }
    names
}
