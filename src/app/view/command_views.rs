//! UI: the Command tab of the remote picker: a Remote Command typed or filled
//! from a Command Preset, shown in full before it is added.

use super::AppView;
use crate::app::remote_command::PresetKind;
use gpui::prelude::FluentBuilder as _;
use gpui::{ElementId, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, Selectable as _, Sizable as _, h_flex, v_flex};

fn label(text: &str) -> gpui::Div {
    div().w(px(88.0)).flex_shrink_0().text_sm().child(text.to_string())
}

impl AppView {
    /// What the Command tab shows under the Server buttons.
    pub(super) fn render_command_tab(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(picker) = self.remote.picker.as_ref() else { return div().into_any_element() };
        let theme = cx.theme();
        let (muted, danger, warning, border) = (theme.muted_foreground, theme.danger, theme.warning, theme.border);
        let preset = picker.command.preset;
        let suggestions = picker.command.suggestions.clone();
        let suggestions_error = picker.command.suggestions_error.clone();
        let loading = picker.command.suggestions_loading;
        let inputs = picker.command.inputs.as_ref().map(|i| {
            (i.command.clone(), i.subject.clone(), i.namespace.clone(), i.container.clone(), i.context.clone(), i.lines.clone())
        });
        let server = self.command_server();
        let error = self.command_form_error(cx);
        let edited = self.command_is_edited(cx);
        let command_text = self.command_text(cx);

        let mut presets = h_flex().gap_1().flex_wrap();
        presets = presets.child(
            Button::new("command-preset-typed")
                .xsmall()
                .label("Type a command")
                .selected(preset.is_none())
                .on_click(cx.listener(|this, _, _, cx| this.command_select_preset(None, cx))),
        );
        for kind in [PresetKind::Journal, PresetKind::Docker, PresetKind::Kube] {
            presets = presets.child(
                Button::new(SharedString::from(format!("command-preset-{}", kind.label())))
                    .xsmall()
                    .label(kind.label())
                    .selected(preset == Some(kind))
                    .on_click(cx.listener(move |this, _, _, cx| this.command_select_preset(Some(kind), cx))),
            );
        }

        let Some((command, subject, namespace, container, context, lines)) = inputs else {
            // The fields are made on the first render of the tab.
            return v_flex().gap_2().child(presets).into_any_element();
        };

        let mut body = v_flex().gap_2().child(h_flex().gap_2().items_start().child(label("Preset")).child(presets));
        if let Some(kind) = preset {
            let mut found = h_flex().gap_1().flex_wrap();
            for (ix, name) in suggestions.iter().take(40).enumerate() {
                let pick = name.clone();
                found = found.child(
                    Button::new(ElementId::from(("command-suggestion", ix)))
                        .xsmall()
                        .ghost()
                        .label(name.clone())
                        .on_click(cx.listener(move |this, _, window, cx| this.command_pick_suggestion(&pick, window, cx))),
                );
            }
            body = body
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(label(kind.subject()))
                        .child(div().flex_1().child(Input::new(&subject)))
                        .child(
                            Button::new("command-refresh")
                                .xsmall()
                                .label(if loading { "Listing…" } else { "Refresh" })
                                .on_click(cx.listener(|this, _, _, cx| this.command_load_suggestions(cx))),
                        ),
                )
                .when_some(suggestions_error, |this, e| {
                    this.child(h_flex().gap_2().child(label("")).child(div().flex_1().text_xs().text_color(danger).child(e)))
                })
                .when(!suggestions.is_empty(), |this| {
                    this.child(h_flex().gap_2().items_start().child(label("")).child(div().flex_1().child(found)))
                })
                .when(kind == PresetKind::Kube, |this| {
                    this.child(h_flex().gap_2().items_center().child(label("Namespace")).child(div().flex_1().child(Input::new(&namespace))))
                        .child(h_flex().gap_2().items_center().child(label("Container")).child(div().flex_1().child(Input::new(&container))))
                        .child(h_flex().gap_2().items_center().child(label("Context")).child(div().flex_1().child(Input::new(&context))))
                })
                .child(h_flex().gap_2().items_center().child(label("First lines")).child(div().w(px(96.0)).child(Input::new(&lines))));
        }
        body = body.child(
            h_flex()
                .gap_2()
                .items_center()
                .child(label("Command"))
                .child(div().flex_1().child(Input::new(&command))),
        );
        if preset.is_some() && edited {
            body = body.child(
                h_flex().gap_2().child(label("")).child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(warning)
                        .child("Edited: will rerun as typed after a lost connection"),
                ),
            );
        }
        let runs_on = match (&server, command_text.is_empty()) {
            (Some((_, name, user)), false) => {
                Some(format!("Runs on {name} as {user} each time this Log Space opens"))
            }
            _ => None,
        };
        body.when_some(error, |this, e| {
            this.child(h_flex().gap_2().child(label("")).child(div().flex_1().text_xs().text_color(danger).child(e)))
        })
        .when_some(runs_on, |this, text| {
            this.child(
                h_flex()
                    .gap_2()
                    .child(label(""))
                    .child(div().flex_1().p_2().rounded_md().border_1().border_color(border).text_xs().text_color(muted).child(text)),
            )
        })
        .into_any_element()
    }
}
