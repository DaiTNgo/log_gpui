use crate::parser::event::LogEvent;

/// Wraps the shared `QueryEngine` and returns the indices of visible events.
pub struct FilterEngine;

impl FilterEngine {
    pub fn get_visible(events: &[LogEvent], query: &crate::query::ast::QueryExpression) -> Vec<usize> {
        let mut visible = Vec::new();
        for (idx, event) in events.iter().enumerate() {
            if crate::query::engine::QueryEngine::evaluate_event(event, query) {
                visible.push(idx);
            }
        }
        visible
    }
}