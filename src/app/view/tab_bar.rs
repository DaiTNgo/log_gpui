//! UI: working-set tab bar built from gpui-kit `TabBar` / `Tab`.

use super::AppView;
use crate::workspace::working_set::TabTarget;
use gpui::{Context, IntoElement, Styled};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{Sizable as _};
use gpui_kit::assets::IconName;

impl AppView {
    pub(super) fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let view = cx.entity();
        let (labels, active_idx): (Vec<String>, Option<usize>) = match self.active_project() {
            Some(proj) => (
                proj.working_set
                    .tabs()
                    .iter()
                    .map(|tab| match &tab.target {
                        TabTarget::LogSpace { space_id } => proj
                            .log_space(space_id)
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| space_id.clone()),
                        TabTarget::FileView { space_id, file_id } => {
                            let space = proj.log_space(space_id);
                            let file = space.and_then(|s| s.file(file_id));
                            match (file, space) {
                                (Some(f), Some(s)) => format!("{} · {}", f.display_name, s.name),
                                _ => file_id.clone(),
                            }
                        }
                        TabTarget::InvestigationBoard { case_id } => format!("⚑ {}", case_id),
                    })
                    .collect(),
                proj.working_set.active_tab_index(),
            ),
            None => (Vec::new(), None),
        };

        let tabs = labels.into_iter().enumerate().map(|(index, label)| {
            let close_view = view.clone();
            Tab::new().label(label).suffix(
                Button::new(("close-tab", index))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .on_click(move |_, _, app| {
                        app.stop_propagation();
                        close_view.update(app, |this, cx| this.close_tab_at(index, cx));
                    }),
            )
        });

        TabBar::new("working-set")
            .bg(crate::ui::main_view::background(cx))
            .selected_index(active_idx.unwrap_or(0))
            .on_click(cx.listener(|this, ix: &usize, _window, cx| this.switch_to_tab(*ix, cx)))
            .children(tabs)
            .into_any_element()
    }
}
