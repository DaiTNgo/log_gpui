//! Logic: density, columns, sidebar and drawer state.

use super::AppView;
use crate::density::Density;
use gpui::{
    Context,
};

impl AppView {
    /// Left sidebar (Projects): `⌘B` / header toggle.
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.toggle();
        cx.notify();
    }

    /// Right context sidebar (View / Filter / Marker / Inspector).
    pub fn toggle_context_sidebar(&mut self, cx: &mut Context<Self>) {
        self.context_sidebar.toggle();
        cx.notify();
    }

    pub fn toggle_context_section(
        &mut self,
        section: crate::workspace::state::ContextSection,
        cx: &mut Context<Self>,
    ) {
        self.context_sidebar.toggle_section(section);
        cx.notify();
    }

    /// Shortcut behaviour: collapse `section` when it is showing, otherwise
    /// open the sidebar with it expanded.
    pub fn toggle_or_reveal_context_section(
        &mut self,
        section: crate::workspace::state::ContextSection,
        cx: &mut Context<Self>,
    ) {
        let showing = self.context_sidebar.is_shown(self.active_tab_is_log_file())
            && !self.context_sidebar.is_collapsed(section);
        if showing {
            self.context_sidebar.toggle_section(section);
        } else {
            self.context_sidebar.reveal(section);
        }
        cx.notify();
    }

    /// Toolbar shortcut: open the sidebar with `section` expanded.
    pub fn reveal_context_section(
        &mut self,
        section: crate::workspace::state::ContextSection,
        cx: &mut Context<Self>,
    ) {
        self.context_sidebar.reveal(section);
        cx.notify();
    }

    pub fn toggle_project_expand(&mut self, project_id: &str, cx: &mut Context<Self>) {
        if self.expanded_projects.contains(project_id) {
            self.expanded_projects.remove(project_id);
        } else {
            self.expanded_projects.insert(project_id.to_string());
        }
        cx.notify();
    }

    /// Header action: collapse every project, or expand all when none is expanded.
    pub fn toggle_all_projects(&mut self, cx: &mut Context<Self>) {
        if self.expanded_projects.is_empty() {
            self.expand_all_projects(cx);
        } else {
            self.collapse_all_projects(cx);
        }
    }

    pub fn collapse_all_projects(&mut self, cx: &mut Context<Self>) {
        self.expanded_projects.clear();
        cx.notify();
    }

    pub fn expand_all_projects(&mut self, cx: &mut Context<Self>) {
        for project in &self.workspace_state.projects {
            self.expanded_projects.insert(project.id.clone());
        }
        cx.notify();
    }

    pub fn set_density(&mut self, density: Density, cx: &mut Context<Self>) {
        self.density = density;
        self.column_layout.update_density(self.density);
        self.workspace_state.density = density;
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn set_show_minimap(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_minimap = show;
        self.workspace_state.show_minimap = show;
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Search Highlight: whether the Focus Pane paints Search Matches. One
    /// setting for every Focus Tab and Focus Pane.
    pub fn set_show_search_highlight(&mut self, show: bool, cx: &mut Context<Self>) {
        self.workspace_state.show_search_highlight = show;
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn toggle_search_highlight(&mut self, cx: &mut Context<Self>) {
        let show = !self.workspace_state.show_search_highlight;
        self.set_show_search_highlight(show, cx);
    }

    /// Whether Search Matches are painted per Phrase (the Settings toggle).
    /// The Canvas, minimap, Focus Pane and Raw View read it here.
    pub fn phrase_colors(&self) -> bool {
        self.workspace_state.phrase_colors
    }

    pub fn set_phrase_colors(&mut self, on: bool, cx: &mut Context<Self>) {
        self.workspace_state.phrase_colors = on;
        self.update_raw_highlights(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn toggle_pin_column(&mut self, col: crate::column::PinnableColumn, cx: &mut Context<Self>) {
        self.column_layout.toggle_pin(col);
        cx.notify();
    }

    pub fn unpin_all_columns(&mut self, cx: &mut Context<Self>) {
        self.column_layout.unpin_all();
        cx.notify();
    }

    pub fn set_column_header_hovered(
        &mut self,
        col: crate::column::PinnableColumn,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        self.column_layout.set_header_hovered(col, hovered);
        cx.notify();
    }

    pub fn set_column_toggle_hovered(
        &mut self,
        col: crate::column::PinnableColumn,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        self.column_layout.set_toggle_hovered(col, hovered);
        cx.notify();
    }

    pub fn start_resizing_column(
        &mut self,
        col: crate::column::PinnableColumn,
        start_x: gpui::Pixels,
        cx: &mut Context<Self>,
    ) {
        self.column_layout.start_resizing(col, start_x);
        cx.notify();
    }

    pub fn update_column_resize(&mut self, current_x: gpui::Pixels, cx: &mut Context<Self>) {
        self.column_layout.update_resizing(current_x);
        cx.notify();
    }

    pub fn finish_resizing_column(&mut self, cx: &mut Context<Self>) {
        self.column_layout.finish_resizing();
        cx.notify();
    }

    pub fn auto_fit_column(&mut self, col: crate::column::PinnableColumn, cx: &mut Context<Self>) {
        self.column_layout.auto_fit(col, self.density);
        cx.notify();
    }

    pub fn start_resizing_drawer(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.is_resizing = true;
        cx.notify();
    }

    pub fn update_drawer_resize(&mut self, current_x: gpui::Pixels, cx: &mut Context<Self>) {
        self.sidebar_state.set_width(f32::from(current_x));
        cx.notify();
    }

    pub fn finish_resizing_drawer(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.is_resizing = false;
        cx.notify();
    }

    pub fn reset_drawer_width(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.reset_width();
        cx.notify();
    }
}

/// Which kind of match a color setting paints.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Search,
    Find,
}

impl AppView {
    /// Whether the focused tab is a Log Space (the Source Column applies).
    pub fn active_is_log_space(&self) -> bool {
        matches!(
            self.active_target(),
            Some(crate::workspace::working_set::TabTarget::LogSpace { .. })
        )
    }

    /// Set the hue of a kind of match; `None` goes back to the theme's color.
    pub fn set_match_color(&mut self, kind: MatchKind, hue: Option<crate::palette::Hue>, cx: &mut Context<Self>) {
        match kind {
            MatchKind::Search => self.workspace_state.match_colors.search = hue,
            MatchKind::Find => self.workspace_state.match_colors.find = hue,
        }
        self.schedule_session_save(cx);
        cx.notify();
    }
}
