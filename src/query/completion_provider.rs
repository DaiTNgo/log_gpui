use crate::query::language_service::{CompletionKind, QueryLanguageService};
use crate::query::schema::LogSchema;
use anyhow::Result;
use gpui::{App, Task, Window};
use gpui_component::input::{CompletionProvider, Rope};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, InsertTextFormat,
};

/// LSP-style completion provider for the query language, backed by
/// [`QueryLanguageService`]. Attached to the search editor's `lsp_mut()`
/// so the native completion popover renders suggestions as the user types.
pub struct QueryCompletionProvider {
    schema: LogSchema,
}

impl QueryCompletionProvider {
    pub fn new(schema: LogSchema) -> Self {
        Self { schema }
    }
}

fn lsp_kind(kind: CompletionKind) -> CompletionItemKind {
    match kind {
        CompletionKind::Field => CompletionItemKind::FIELD,
        CompletionKind::Operator | CompletionKind::LogicalOp => CompletionItemKind::OPERATOR,
        CompletionKind::Value => CompletionItemKind::ENUM_MEMBER,
    }
}

impl CompletionProvider for QueryCompletionProvider {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let text_str = text.to_string();
        let items = QueryLanguageService::get_completions(&text_str, offset, &self.schema);
        let completions: Vec<CompletionItem> = items
            .into_iter()
            .map(|c| CompletionItem {
                label: c.label.into(),
                detail: c.detail.map(Into::into),
                kind: Some(lsp_kind(c.kind)),
                insert_text: Some(c.insert_text.into()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..CompletionItem::default()
            })
            .collect();
        Task::ready(Ok(CompletionResponse::Array(completions)))
    }

    fn is_completion_trigger(&self, _offset: usize, _new_text: &str, _cx: &mut App) -> bool {
        true
    }
}