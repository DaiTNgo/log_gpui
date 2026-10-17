//! Logic: saved views and investigation cases.

use super::AppView;
use crate::workspace::working_set::TabSessionState;
use gpui::{
    Context,
};

impl AppView {
    pub fn apply_saved_view(&mut self, view_id: &str, cx: &mut Context<Self>) {
        if let Some(proj) = self.workspace_state.active_project() {
            if let Some(view) = proj.saved_views.iter().find(|v| v.id == view_id).cloned() {
                let mut session = TabSessionState::new();
                session.filter = self.filter_engine.clone();
                session.search = self.search_state.clone();
                session.column_layout = self.column_layout.clone();
                view.apply_to_session(&mut session);
                self.filter_engine = session.filter;
                self.search_state = session.search;
                self.column_layout = session.column_layout;
                self.apply_filters_and_refresh(cx);
                cx.notify();
            }
        }
    }

    pub fn save_current_view(&mut self, cx: &mut Context<Self>) {
        let mut session = TabSessionState::new();
        session.filter = self.filter_engine.clone();
        session.search = self.search_state.clone();
        session.column_layout = self.column_layout.clone();
        let view_num = self
            .workspace_state
            .active_project()
            .map(|p| p.saved_views.len() + 1)
            .unwrap_or(1);
        let id = format!("view-{}", view_num);
        let name = format!("Preset Filter #{}", view_num);
        let view = crate::workspace::saved_view::SavedView::capture(id, name, &session);
        if let Some(proj) = self.workspace_state.active_project_mut() {
            proj.saved_views.push(view);
            cx.notify();
        }
    }

    pub fn create_investigation_case(&mut self, cx: &mut Context<Self>) {
        let case_num = self
            .workspace_state
            .active_project()
            .map(|p| p.investigation_cases.len() + 1)
            .unwrap_or(1);
        let id = format!("INC-{}", case_num);
        let title = format!("Incident #{}", case_num);
        let case = crate::workspace::investigation::InvestigationCase::new(id, title, "P1");
        if let Some(proj) = self.workspace_state.active_project_mut() {
            proj.investigation_cases.push(case);
            cx.notify();
        }
    }

    pub fn open_investigation_case(&mut self, case_id: &str, cx: &mut Context<Self>) {
        if let Some(proj) = self.active_project_mut() {
            proj.open_investigation_board(case_id);
            self.restore_active_session(cx);
            cx.notify();
        }
    }
}
