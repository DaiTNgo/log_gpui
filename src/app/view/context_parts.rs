//! UI: the parts of the context sidebar's sections built from the app: each
//! section's body, the summary on its header and the header's buttons.

use super::AppView;
use crate::ui::context_sidebar::{ContextSections, SectionParts};
use crate::ui::inspector::InspectorView;
use crate::workspace::state::InspectorMode;
use crate::workspace::working_set::ViewMode;
use gpui::{Context, IntoElement, ParentElement, Styled};
use gpui_component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_component::{Selectable as _, Sizable as _, h_flex};
use gpui_kit::assets::IconName;

impl AppView {
    /// A small header button; clicking it does not fold the section.
    fn header_button(
        id: &'static str,
        icon: IconName,
        tooltip: &'static str,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut AppView, &mut Context<AppView>) + 'static,
    ) -> Button {
        Button::new(id).ghost().xsmall().icon(icon).tooltip(tooltip).on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            on_click(this, cx)
        }))
    }

    pub(super) fn context_sections(&mut self, cx: &mut Context<Self>) -> ContextSections {
        let parsed = self.view_mode == ViewMode::Parsed;

        let active = self.filter_state.active_count();
        let filter_summary = match (active, self.filter_state.paused) {
            (0, _) => None,
            (n, false) => Some(format!("{n} active")),
            (n, true) => Some(format!("{n} active · paused")),
        };
        let mut filter_actions = h_flex().gap_0p5();
        if active > 0 {
            let paused = self.filter_state.paused;
            filter_actions = filter_actions.child(
                Button::new("ctx-filter-pause")
                    .ghost()
                    .xsmall()
                    .label(if paused { "Resume" } else { "Pause" })
                    .tooltip(if paused { "Apply the Filter again" } else { "Let every line into the Focus Pane, keeping the conditions" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_filter_paused(cx)
                    })),
            );
        }
        filter_actions = filter_actions.child(Self::header_button("ctx-filter-add", IconName::Plus, "Add a filter", cx, |this, cx| {
            this.context_sidebar.reveal(crate::workspace::ContextSection::Filter);
            this.toggle_filter_picker(cx)
        }));
        let filter = SectionParts {
            body: self.render_filter_panel_content(cx),
            summary: filter_summary,
            actions: Some(filter_actions.into_any_element()),
        };

        let rules = self.highlight_rules();
        let on = rules.iter().filter(|r| r.enabled).count();
        let highlights_summary = match (rules.len(), on) {
            (0, _) => None,
            (n, on) if on == n => Some(if n == 1 { "1 rule".to_string() } else { format!("{n} rules") }),
            (n, on) => Some(format!("{on} of {n} on")),
        };
        let highlights = SectionParts {
            body: self.render_highlights_panel_content(cx),
            summary: highlights_summary,
            actions: Some(
                Self::header_button("ctx-highlights-add", IconName::Plus, "New rule", cx, |this, cx| {
                    this.context_sidebar.reveal(crate::workspace::ContextSection::Highlights);
                    this.open_rule_form(cx)
                })
                .into_any_element(),
            ),
        };

        let groups = self.column_groups("");
        let rows = groups.built_in.iter().chain(&groups.from_format).chain(&groups.detected);
        let (shown, total) = rows.fold((0, 0), |(s, t), r| (s + usize::from(r.shown), t + 1));
        let columns = SectionParts {
            body: self.render_columns_panel_content(cx),
            summary: (total > 0).then(|| format!("{shown} of {} shown", total + groups.detected_more)),
            actions: None,
        };

        let mode = self.workspace_state.context_layout.inspector_mode;
        let fields = self.inspector_fields();
        let has_line = self.inspector_text.is_some();
        let show_fields = mode == InspectorMode::Fields && !fields.is_empty();
        let body = if show_fields {
            InspectorView::render_fields(&fields, parsed, cx)
        } else {
            InspectorView::render_raw(has_line, &self.inspector_editor, cx)
        };
        let summary = self.current_line().filter(|_| has_line).map(|l| format!("line {}", l.line));
        let mut actions = h_flex().gap_0p5();
        if parsed && !fields.is_empty() {
            actions = actions.child(
                ButtonGroup::new("ctx-inspector-mode")
                    .outline()
                    .xsmall()
                    .child(Button::new("ctx-inspector-fields").label("Fields").selected(mode == InspectorMode::Fields))
                    .child(Button::new("ctx-inspector-raw").label("Raw").selected(mode == InspectorMode::Raw))
                    .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                        cx.stop_propagation();
                        let mode = if selected.first() == Some(&1) { InspectorMode::Raw } else { InspectorMode::Fields };
                        this.set_inspector_mode(mode, cx)
                    })),
            );
        }
        if has_line {
            actions = actions
                .child(Self::header_button("ctx-inspector-copy", IconName::Copy, "Copy", cx, |this, cx| this.copy_inspector(cx)));
        }
        let inspector = SectionParts { body, summary, actions: Some(actions.into_any_element()) };

        ContextSections { filter, highlights, columns, inspector, parsed }
    }
}
