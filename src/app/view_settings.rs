//! Logic: density, tail, columns, sidebar and drawer state.

use super::AppView;
use crate::theme::Density;
use gpui::{
    Context,
};

impl AppView {
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.toggle_drawer();
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
        cx.notify();
    }

    pub fn toggle_tail(&mut self, cx: &mut Context<Self>) {
        self.tail_enabled = !self.tail_enabled;
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
        let new_w = f32::from(current_x) - crate::ui::sidebar::RAIL_WIDTH;
        self.sidebar_state.set_drawer_width(new_w);
        cx.notify();
    }

    pub fn finish_resizing_drawer(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.is_resizing = false;
        cx.notify();
    }

    pub fn reset_drawer_width(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.reset_drawer_width();
        cx.notify();
    }
}
