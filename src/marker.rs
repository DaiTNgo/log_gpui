use crate::model::LogLevel;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarkerType {
    Error,
    Warning,
    Bookmark,
    Important,
    Note,
}

impl MarkerType {
    pub fn symbol(&self) -> &'static str {
        match self {
            MarkerType::Error => "●",
            MarkerType::Warning => "▲",
            MarkerType::Bookmark => "◆",
            MarkerType::Important => "⭐",
            MarkerType::Note => "📌",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            MarkerType::Error => "Error",
            MarkerType::Warning => "Warning",
            MarkerType::Bookmark => "Bookmark",
            MarkerType::Important => "Important",
            MarkerType::Note => "Note",
        }
    }

    pub fn from_log_level(level: LogLevel) -> Option<Self> {
        match level {
            LogLevel::Fatal | LogLevel::Error => Some(MarkerType::Error),
            LogLevel::Warn => Some(MarkerType::Warning),
            _ => None,
        }
    }

    pub fn cycle(current: Option<Self>) -> Option<Self> {
        match current {
            None => Some(MarkerType::Bookmark),
            Some(MarkerType::Bookmark) => Some(MarkerType::Important),
            Some(MarkerType::Important) => Some(MarkerType::Note),
            Some(MarkerType::Note) => None,
            Some(_) => Some(MarkerType::Bookmark),
        }
    }
}

/// An entry in the overview ruler (minimap indicator)
#[derive(Debug, Clone)]
pub struct RulerMark {
    /// Normalized position in file: 0.0 ..= 1.0
    pub position: f32,
    /// Absolute row index in the all_entries
    pub entry_index: usize,
    /// Type for tooltip / semantics
    pub marker_type: Option<MarkerType>,
    /// Whether this is a search result match
    pub is_search: bool,
    /// Whether this is the currently selected search match
    pub is_current_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkerInfo {
    pub marker_type: MarkerType,
    pub note: Option<String>,
    pub created_at: Option<String>,
}
