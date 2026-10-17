use crate::model::LogLevel;
use serde::{Deserialize, Serialize};

/// Lifecycle status of an Investigation Case tracking investigative progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CaseStatus {
    Active,
    RootCauseIdentified,
    Mitigated,
    Archived,
}

impl CaseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaseStatus::Active => "Active",
            CaseStatus::RootCauseIdentified => "Root Cause Identified",
            CaseStatus::Mitigated => "Mitigated",
            CaseStatus::Archived => "Archived",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "root cause identified" | "root" => CaseStatus::RootCauseIdentified,
            "mitigated" => CaseStatus::Mitigated,
            "archived" => CaseStatus::Archived,
            _ => CaseStatus::Active,
        }
    }
}

/// A cross-file evidence anchor: an exact log line pinned into an
/// investigation case for chronological correlation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAnchor {
    /// Unique anchor identifier.
    pub id: String,
    /// Id of the log file this anchor points into.
    pub file_id: String,
    /// 1-based line number within the source file.
    pub line_number: usize,
    /// Unix timestamp (seconds, subsecond precision) for chronological sorting.
    pub timestamp: f64,
    /// Severity of the anchored entry.
    pub level: LogLevel,
    /// Raw/payload excerpt preserved at anchor time.
    pub payload: String,
    /// Optional markdown diagnostic note attached to this anchor.
    pub note: Option<String>,
}

/// An incident-centric investigation case pooling evidence anchors
/// gathered across disparate log files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationCase {
    /// Case identifier, e.g. "INC-142".
    pub id: String,
    /// Human readable title, e.g. "Payment Gateway Timeout".
    pub title: String,
    /// Severity label.
    pub severity: String,
    /// Current lifecycle status.
    pub status: CaseStatus,
    /// Chronology of anchored log lines.
    pub anchors: Vec<LogAnchor>,
    /// Case-level diagnostic markdown notes.
    pub notes: Vec<String>,
}

impl InvestigationCase {
    pub fn new(id: String, title: String, severity: &str) -> Self {
        Self {
            id,
            title,
            severity: severity.to_string(),
            status: CaseStatus::Active,
            anchors: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn display_name(&self) -> String {
        format!("{}: {}", self.id, self.title)
    }

    /// Anchor a log line into this case. Returns the anchor id if added.
    pub fn anchor_line(
        &mut self,
        anchor_id: String,
        file_id: &str,
        line_number: usize,
        timestamp: f64,
        level: LogLevel,
        payload: String,
        note: Option<String>,
    ) -> String {
        self.anchors.push(LogAnchor {
            id: anchor_id.clone(),
            file_id: file_id.to_string(),
            line_number,
            timestamp,
            level,
            payload,
            note,
        });
        anchor_id
    }

    /// Remove an anchor by id. Returns true if removed.
    pub fn remove_anchor(&mut self, anchor_id: &str) -> bool {
        let before = self.anchors.len();
        self.anchors.retain(|a| a.id != anchor_id);
        self.anchors.len() != before
    }

    /// Attach a markdown note to a specific anchor.
    pub fn set_anchor_note(&mut self, anchor_id: &str, note: Option<String>) -> bool {
        if let Some(anchor) = self.anchors.iter_mut().find(|a| a.id == anchor_id) {
            anchor.note = note;
            true
        } else {
            false
        }
    }

    /// Append a case-level diagnostic note.
    pub fn add_case_note(&mut self, note: String) {
        self.notes.push(note);
    }

    /// The unified chronological evidence trail: all anchors sorted strictly
    /// by timestamp (stable for equal timestamps).
    pub fn chronological_trail(&self) -> Vec<&LogAnchor> {
        let mut sorted: Vec<&LogAnchor> = self.anchors.iter().collect();
        sorted.sort_by(|a, b| {
            a.timestamp
                .partial_cmp(&b.timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    }

    /// Count of anchors originating from a given file.
    pub fn anchor_count_for_file(&self, file_id: &str) -> usize {
        self.anchors.iter().filter(|a| a.file_id == file_id).count()
    }

    pub fn is_archived(&self) -> bool {
        self.status == CaseStatus::Archived
    }

    /// Export a structured post-mortem markdown summary of this case,
    /// including incident metadata and exact log excerpts.
    pub fn export_post_mortem_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}: {}\n\n", self.id, self.title));
        md.push_str(&format!("- **Status**: {}\n", self.status.as_str()));
        md.push_str(&format!("- **Severity**: {}\n", self.severity));
        md.push_str(&format!("- **Evidence entries**: {}\n", self.anchors.len()));

        if !self.notes.is_empty() {
            md.push_str("\n## Diagnostic Notes\n\n");
            for (i, note) in self.notes.iter().enumerate() {
                md.push_str(&format!("{}. {}\n", i + 1, note));
            }
        }

        md.push_str("\n## Chronological Evidence Trail\n\n");
        for anchor in self.chronological_trail() {
            md.push_str("### Anchor\n");
            md.push_str(&format!(
                "- **File**: `{}` (line {})\n",
                anchor.file_id, anchor.line_number
            ));
            md.push_str(&format!(
                "- **Timestamp**: {:.3}\n",
                anchor.timestamp
            ));
            md.push_str(&format!("- **Level**: {}\n", anchor.level.as_str()));
            md.push_str("```log\n");
            md.push_str(&anchor.payload);
            md.push('\n');
            md.push_str("```\n");
            if let Some(note) = &anchor.note {
                md.push_str(&format!("\n> {}\n", note));
            }
            md.push('\n');
        }
        md
    }
}

/// Monotonic case-id generator producing INC-<n> identifiers.
#[derive(Debug, Clone, Default)]
pub struct CaseIdGenerator {
    next: u32,
}

impl CaseIdGenerator {
    pub fn new(start: u32) -> Self {
        Self { next: start }
    }

    /// Generate the next case id given already-existing case ids, ensuring
    /// no collision with registered cases.
    pub fn next(&mut self, existing: &[&InvestigationCase]) -> String {
        loop {
            let candidate = format!("INC-{}", self.next);
            self.next += 1;
            if !existing.iter().any(|c| c.id == candidate) {
                return candidate;
            }
        }
    }
}