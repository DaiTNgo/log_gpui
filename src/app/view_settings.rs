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

    /// The size the log text is drawn at.
    pub fn text_scale(&self) -> crate::density::TextScale {
        crate::density::TextScale::new(self.density, self.text_zoom)
    }

    /// Make the log text larger (`steps` > 0) or smaller, within bounds.
    pub fn zoom_text(&mut self, steps: i8, cx: &mut Context<Self>) {
        self.set_text_zoom(self.text_zoom.saturating_add(steps), cx);
    }

    pub fn set_text_zoom(&mut self, zoom: i8, cx: &mut Context<Self>) {
        let zoom = crate::density::TextScale::new(self.density, zoom).zoom;
        if zoom == self.text_zoom {
            return;
        }
        self.text_zoom = zoom;
        self.workspace_state.text_zoom = zoom;
        self.column_layout.update_density(self.text_scale());
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn set_density(&mut self, density: Density, cx: &mut Context<Self>) {
        self.density = density;
        self.column_layout.update_density(self.text_scale());
        self.workspace_state.density = density;
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn set_theme(&mut self, theme: crate::theme::AppTheme, cx: &mut Context<Self>) {
        self.workspace_state.theme = theme;
        theme.apply(cx);
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

    /// Value Colors: whether identifier values get their color in the
    /// Parsed View and the Focus Pane. One app setting.
    pub fn set_show_value_colors(&mut self, show: bool, cx: &mut Context<Self>) {
        self.workspace_state.show_value_colors = show;
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn toggle_value_colors(&mut self, cx: &mut Context<Self>) {
        let show = !self.workspace_state.show_value_colors;
        self.set_show_value_colors(show, cx);
    }

    /// Syntax Color: whether log lines are painted per token. One setting for
    /// every tab.
    pub fn set_show_syntax_color(&mut self, show: bool, cx: &mut Context<Self>) {
        self.workspace_state.show_syntax_color = show;
        self.update_raw_highlights(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn toggle_syntax_color(&mut self, cx: &mut Context<Self>) {
        let show = !self.workspace_state.show_syntax_color;
        self.set_show_syntax_color(show, cx);
    }

    /// Whether Search Matches are painted per Phrase (the Settings toggle).
    /// The Canvas, minimap, Focus Pane and Raw View read it here.
    pub fn phrase_colors(&self) -> bool {
        self.workspace_state.phrase_colors
    }

    /// Choose the hue of a Phrase (`None`: back to the generated color).
    pub fn set_phrase_hue(&mut self, key: &str, hue: Option<f32>, cx: &mut Context<Self>) {
        self.workspace_state.phrase_rgbs.remove(key);
        match hue {
            Some(h) => self.workspace_state.phrase_hues.insert(key.to_string(), h),
            None => self.workspace_state.phrase_hues.remove(key),
        };
        self.apply_phrase_colors(cx);
    }

    /// Paint a Phrase in exactly this color (`0xRRGGBB`).
    pub fn set_phrase_rgb(&mut self, key: &str, rgb: u32, cx: &mut Context<Self>) {
        self.workspace_state.phrase_hues.remove(key);
        self.workspace_state.phrase_rgbs.insert(key.to_string(), rgb);
        self.apply_phrase_colors(cx);
    }

    fn apply_phrase_colors(&mut self, cx: &mut Context<Self>) {
        crate::search_color::set_hue_overrides(self.workspace_state.phrase_hues.clone());
        crate::search_color::set_rgb_overrides(self.workspace_state.phrase_rgbs.clone());
        crate::search_color::set_preview(None);
        self.update_raw_highlights(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Show a color on a Phrase while the pointer is over it in the menu;
    /// `None` stops the preview.
    pub fn preview_phrase(&mut self, key: &str, preview: Option<crate::search_color::Preview>, cx: &mut Context<Self>) {
        crate::search_color::set_preview(preview.map(|p| (key.to_string(), p)));
        self.update_raw_highlights(cx);
        cx.notify();
    }

    /// Close the color menu and drop any preview it was showing.
    pub fn close_phrase_menu(&mut self, cx: &mut Context<Self>) {
        self.phrase_menu = None;
        crate::search_color::set_preview(None);
        self.update_raw_highlights(cx);
        cx.notify();
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
        self.column_layout.auto_fit(col, self.text_scale());
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

impl AppView {
    /// Whether the focused tab is a Log Space (the Source Column applies).
    pub fn active_is_log_space(&self) -> bool {
        matches!(
            self.active_target(),
            Some(crate::workspace::working_set::TabTarget::LogSpace { .. })
        )
    }
}
