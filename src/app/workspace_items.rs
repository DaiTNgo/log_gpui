//! Logic: investigation cases.

use super::AppView;
use gpui::{
    Context,
};

impl AppView {
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
