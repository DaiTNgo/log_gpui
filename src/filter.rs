use crate::model::{LogEntry, LogLevel};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    /// Only messages matching ANY include filter will be displayed (if at least one include filter is active)
    Include,
    /// Messages matching ANY exclude filter will be hidden (takes precedence)
    Exclude,
    /// Messages matching marker filter are highlighted with color without being hidden
    Marker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterRule {
    pub id: usize,
    pub name: String,
    pub enabled: bool,
    pub filter_type: FilterType,
    pub tag: Option<String>,
    pub min_level: Option<LogLevel>,
    pub max_level: Option<LogLevel>,
    pub exact_level: Option<LogLevel>,
    pub payload_query: Option<String>,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub marker_color: Option<String>,
    #[serde(skip)]
    pub hit_count: usize,
}

impl FilterRule {
    pub fn new(id: usize, name: String, filter_type: FilterType) -> Self {
        Self {
            id,
            name,
            enabled: true,
            filter_type,
            tag: None,
            min_level: None,
            max_level: None,
            exact_level: None,
            payload_query: None,
            is_regex: false,
            case_sensitive: false,
            marker_color: None,
            hit_count: 0,
        }
    }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        if !self.enabled {
            return false;
        }

        // Match Tag
        if let Some(ref tag) = self.tag {
            if !tag.is_empty() && !tag.eq_ignore_ascii_case(&entry.tag) && tag != "*" {
                return false;
            }
        }

        // Match Exact Level
        if let Some(exact_level) = self.exact_level {
            if entry.level != exact_level {
                return false;
            }
        }

        // Match Min Level
        if let Some(min_level) = self.min_level {
            if entry.level < min_level && entry.level != LogLevel::Unknown {
                return false;
            }
        }

        // Match Max Level
        if let Some(max_level) = self.max_level {
            if entry.level > max_level && entry.level != LogLevel::Unknown {
                return false;
            }
        }

        // Match Payload
        if let Some(ref query) = self.payload_query {
            if !query.is_empty() {
                if self.is_regex {
                    if let Ok(re) = Regex::new(query) {
                        if !re.is_match(&entry.payload) && !re.is_match(&entry.raw_line) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                } else {
                    if self.case_sensitive {
                        if !entry.payload.contains(query) && !entry.raw_line.contains(query) {
                            return false;
                        }
                    } else {
                        let query_lower = query.to_lowercase();
                        if !entry.payload.to_lowercase().contains(&query_lower)
                            && !entry.raw_line.to_lowercase().contains(&query_lower)
                        {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }
}

pub struct FilterEngine;

impl FilterEngine {
    pub fn apply_filters(
        rules: &mut [FilterRule],
        entries: &mut [LogEntry],
        quick_search: &str,
    ) -> Vec<usize> {
        // Reset hit counts
        for rule in rules.iter_mut() {
            rule.hit_count = 0;
        }

        let has_active_include = rules
            .iter()
            .any(|r| r.enabled && r.filter_type == FilterType::Include);

        let active_exclude: Vec<&FilterRule> = rules
            .iter()
            .filter(|r| r.enabled && r.filter_type == FilterType::Exclude)
            .collect();

        let active_include: Vec<&FilterRule> = rules
            .iter()
            .filter(|r| r.enabled && r.filter_type == FilterType::Include)
            .collect();

        let active_markers: Vec<&FilterRule> = rules
            .iter()
            .filter(|r| r.enabled && r.filter_type == FilterType::Marker)
            .collect();

        let quick_search_lower = quick_search.to_lowercase();
        let has_quick_search = !quick_search.trim().is_empty();

        let mut visible_indices = Vec::with_capacity(entries.len());
        let mut prev_timestamp: Option<f64> = None;

        for (idx, entry) in entries.iter_mut().enumerate() {
            entry.marker_color = None;

            // 1. Check Exclude rules (Takes highest priority)
            let mut is_excluded = false;
            for rule in &active_exclude {
                if rule.matches(entry) {
                    is_excluded = true;
                    break;
                }
            }

            if is_excluded {
                continue;
            }

            // 2. Check Include rules
            if has_active_include {
                let mut matched_any_include = false;
                for rule in &active_include {
                    if rule.matches(entry) {
                        matched_any_include = true;
                        break;
                    }
                }
                if !matched_any_include {
                    continue;
                }
            }

            // 3. Check Quick Search
            if has_quick_search {
                let matches_quick = entry.payload.to_lowercase().contains(&quick_search_lower)
                    || entry.tag.to_lowercase().contains(&quick_search_lower)
                    || entry.raw_line.to_lowercase().contains(&quick_search_lower);
                if !matches_quick {
                    continue;
                }
            }

            // 4. Check Markers (highlighting)
            for rule in &active_markers {
                if rule.matches(entry) {
                    if let Some(ref color) = rule.marker_color {
                        entry.marker_color = Some(color.clone());
                        break;
                    }
                }
            }

            // Calculate Delta Time relative to previous visible entry
            if let Some(prev) = prev_timestamp {
                let delta = (entry.timestamp - prev) * 1000.0;
                entry.delta_time_ms = Some(delta);
            } else {
                entry.delta_time_ms = Some(0.0);
            }
            prev_timestamp = Some(entry.timestamp);

            visible_indices.push(idx);
        }

        // Calculate hit counts for all rules
        for rule in rules.iter_mut() {
            if !rule.enabled {
                continue;
            }
            let count = entries.iter().filter(|e| rule.matches(e)).count();
            rule.hit_count = count;
        }

        visible_indices
    }
}
