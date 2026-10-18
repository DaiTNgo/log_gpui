//! Cross-log correlation: find a trace/request identifier across every file,
//! and align files by timestamp.

use std::collections::HashMap;

/// A single hit discovered by a cross-log correlation search.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrelationHit {
    /// Id of the file containing the match.
    pub file_id: String,
    /// 1-based line number of the match within the file.
    pub line_number: usize,
    /// Timestamp of the matching entry (for timestamp alignment).
    pub timestamp: f64,
    /// The matching log level.
    pub level: crate::model::LogLevel,
    /// Matching payload excerpt.
    pub payload: String,
}

/// Cross-log correlation results for a single trace/request identifier,
/// grouped by file with aggregate match counts.
#[derive(Debug, Clone, Default)]
pub struct CorrelationResult {
    /// The trace/request identifier that was searched.
    pub trace_id: String,
    /// Per-file match count badges.
    pub matches_by_file: HashMap<String, usize>,
    /// All hits across every file, sorted chronologically.
    pub hits: Vec<CorrelationHit>,
}

impl CorrelationResult {
    /// Number of files with at least one match.
    pub fn files_matched(&self) -> usize {
        self.matches_by_file.len()
    }

    /// Total number of matching lines across all files.
    pub fn total_hits(&self) -> usize {
        self.hits.len()
    }

    /// Match badge count for a specific file (0 if none).
    pub fn match_count(&self, file_id: &str) -> usize {
        self.matches_by_file.get(file_id).copied().unwrap_or(0)
    }
}

/// Run a case-insensitive substring or regex correlation search for a
/// trace/request identifier across all entries of every registered file.
/// Returns results grouped per file with chronological hits.
///
/// The identifier value (e.g. a request_id) is matched against the raw line,
/// request_id field, trace_id field, and payload of each entry.
pub fn search_across_files(
    trace_id: &str,
    entries_by_file: &std::collections::HashMap<String, Vec<crate::model::LogEntry>>,
    is_regex: bool,
) -> CorrelationResult {
    let mut result = CorrelationResult {
        trace_id: trace_id.to_string(),
        ..Default::default()
    };

    if trace_id.trim().is_empty() {
        return result;
    }

    let re = if is_regex {
        regex::Regex::new(trace_id).ok()
    } else {
        regex::Regex::new(&regex::escape(trace_id)).ok()
    };

    let Some(re) = re else {
        return result;
    };

    for (file_id, entries) in entries_by_file {
        let mut count = 0usize;
        for entry in entries {
            let matches = re.is_match(&entry.raw_line)
                || re.is_match(&entry.payload)
                || entry
                    .request_id
                    .as_deref()
                    .map(|r| re.is_match(r))
                    .unwrap_or(false)
                || entry
                    .trace_id
                    .as_deref()
                    .map(|t| re.is_match(t))
                    .unwrap_or(false);
            if matches {
                count += 1;
                result.hits.push(CorrelationHit {
                    file_id: file_id.clone(),
                    line_number: entry.index,
                    timestamp: entry.timestamp,
                    level: entry.level,
                    payload: entry.payload.clone(),
                });
            }
        }
        if count > 0 {
            result.matches_by_file.insert(file_id.clone(), count);
        }
    }

    result
    .hits
    .sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal));

    result
}

/// Find the index of the entry whose timestamp is closest to `target` within
/// a monotonically increasing timestamp sequence using binary search (O(log N)).
pub fn nearest_timestamp_index(timestamps: &[f64], target: f64) -> Option<usize> {
    if timestamps.is_empty() {
        return None;
    }

    let mut lo = 0usize;
    let mut hi = timestamps.len() - 1;
    if target <= timestamps[lo] {
        return Some(0);
    }
    if target >= timestamps[hi] {
        return Some(hi);
    }

    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if timestamps[mid] <= target {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let dist_lo = (target - timestamps[lo]).abs();
    let dist_hi = (timestamps[hi] - target).abs();
    Some(if dist_lo <= dist_hi { lo } else { hi })
}