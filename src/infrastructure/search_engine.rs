use crate::parser::event::LogEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    pub event_id: usize,
    pub ranges: Vec<(usize, usize)>,
}

/// Wraps the shared `QueryEngine` and produces byte-offset highlight ranges on
/// the raw line of each matching event.
pub struct SearchEngine;

impl SearchEngine {
    pub fn find_matches(events: &[LogEvent], query: &crate::query::ast::QueryExpression) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        for event in events {
            if crate::query::engine::QueryEngine::evaluate_event(event, query) {
                let ranges = crate::query::engine::QueryEngine::find_ranges(event, query);
                matches.push(SearchMatch {
                    event_id: event.index,
                    ranges,
                });
            }
        }
        matches
    }
}