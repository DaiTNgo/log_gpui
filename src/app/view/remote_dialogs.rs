//! UI: the dialogs of remote files: host key confirmation and the password
//! or passphrase prompt.

use super::AppView;
use gpui::prelude::FluentBuilder as _;
use gpui::{AppContext as _, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};

impl AppView {
    /// Create the field of a freshly opened secret prompt and focus it.
    pub fn sync_secret_prompt(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        let Some(prompt) = self.remote.secret_prompt.as_mut() else { return };
        if prompt.input.is_some() {
            return;
        }
        let label = if prompt.passphrase { "Key passphrase" } else { "Password" };
        let input = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder(label));
        cx.subscribe_in(&input, window, |this: &mut Self, input, event: &InputEvent, _window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let secret = input.read(cx).value().to_string();
                this.submit_secret(secret, cx);
            }
        })
        .detach();
        input.update(cx, |state, cx| state.focus(window, cx));
        prompt.input = Some(input);
    }

    /// "Trust this host?": the fingerprint of a key never seen before. The
    /// first question waiting is shown.
    pub(super) fn render_host_key_dialog(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(request) = self.remote.host_prompts.front() else {
            return div().into_any_element();
        };
        let (server, address, fingerprint) =
            (request.server_name.clone(), request.address.clone(), request.fingerprint.clone());
        let theme = cx.theme();
        let (bg, border, muted) = (theme.popover, theme.border, theme.muted_foreground);
        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(480.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(
                        div()
                            .text_base()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!("Trust {server}?")),
                    )
                    .child(div().text_sm().text_color(muted).child(format!(
                        "The authenticity of {address} cannot be checked. Its key fingerprint is:"
                    )))
                    .child(
                        div()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_sm()
                            .child(fingerprint),
                    )
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("hostkey-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.answer_host_key(false, cx))),
                            )
                            .child(
                                Button::new("hostkey-trust")
                                    .primary()
                                    .label("Trust")
                                    .on_click(cx.listener(|this, _, _, cx| this.answer_host_key(true, cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// The password or passphrase of a Server, with "Remember".
    pub(super) fn render_secret_dialog(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(prompt) = self.remote.secret_prompt.as_ref() else {
            return div().into_any_element();
        };
        let (name, passphrase, remember, input, error) = (
            prompt.server_name.clone(),
            prompt.passphrase,
            prompt.remember,
            prompt.input.clone(),
            prompt.error.clone(),
        );
        let theme = cx.theme();
        let (bg, border, danger) = (theme.popover, theme.border, theme.danger);
        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(420.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child(format!(
                        "{} for {name}",
                        if passphrase { "Key passphrase" } else { "Password" }
                    )))
                    .when_some(input, |this, input| this.child(Input::new(&input)))
                    .when_some(error, |this, e| this.child(div().text_xs().text_color(danger).child(e)))
                    .child(
                        Checkbox::new("secret-remember")
                            .label("Remember in Keychain")
                            .checked(remember)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_secret_remember(cx))),
                    )
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("secret-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_secret_prompt(cx))),
                            )
                            .child(
                                Button::new("secret-connect")
                                    .primary()
                                    .small()
                                    .label("Connect")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let secret = this
                                            .remote
                                            .secret_prompt
                                            .as_ref()
                                            .and_then(|p| p.input.as_ref())
                                            .map(|i| i.read(cx).value().to_string())
                                            .unwrap_or_default();
                                        this.submit_secret(secret, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
