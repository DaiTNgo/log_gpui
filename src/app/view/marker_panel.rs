//! UI: marker panel content.

use super::AppView;

use gpui::{
    div, Context, ElementId, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled,
};

impl AppView {
    /// Marker panel: bookmarks & markers navigation list.
    pub fn render_marker_panel_content(
        &mut self,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut list = div().flex().flex_col().gap_1().w_full();
        list = list.child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .mb_2()
                .child("BOOKMARKS & MARKERS"),
        );

        let mut marker_count = 0usize;
        for (vis_pos, &entry_ix) in self.visible_indices.iter().enumerate() {
            if let Some(entry) = self.all_entries.get(entry_ix) {
                if let Some(marker) = entry.effective_marker() {
                    marker_count += 1;
                    let marker_sym = marker.symbol();
                    let target_pos = vis_pos;

                    let item = div()
                        .id(ElementId::from(format!("nav_mark_{}", entry_ix)))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(marker_sym),
                        )
                        .child(
                            div()
                                .text_xs()
                                .child(format!("#{}", entry.index)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .truncate()
                                .child(entry.payload.clone()),
                        )
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.jump_to_row(target_pos, cx);
                        }));

                    list = list.child(item);
                }
            }
        }

        if marker_count == 0 {
            list = list.child(
                div()
                    .py_3()
                    .text_xs()
                    .child("No bookmarks or markers yet. Press B to bookmark the selected line.")
                    .into_any_element(),
            );
        }

        list.into_any_element()
    }
}
