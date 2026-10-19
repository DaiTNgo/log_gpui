//! UI: the Connection Details popover, opened from the connection state in the
//! status bar (the only place connection health is shown, L7).

use super::AppView;
use crate::app::connection_details::{ConnectionSnapshot, MemberAction, MemberDetails, ServerDetails};
use gpui::prelude::FluentBuilder as _;
use gpui::{AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui::StatefulInteractiveElement as _;
use gpui_component::button::Button;
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};

fn fact(label: &str, value: String, muted: gpui::Hsla) -> gpui::Div {
    h_flex()
        .gap_2()
        .text_xs()
        .child(div().w(px(84.0)).flex_shrink_0().text_color(muted).child(label.to_string()))
        .child(div().flex_1().child(value))
}

fn member_block(member: &MemberDetails, server_id: &str, view: &Entity<AppView>, app: &App) -> AnyElement {
    let theme = app.theme();
    let (muted, danger, border) = (theme.muted_foreground, theme.danger, theme.border);
    let file_id = member.file_id.clone();
    let action = member.action;
    let view = view.clone();
    let id = SharedString::from(format!("member-{server_id}-{}", member.file_id));
    let button = match action {
        MemberAction::None => None,
        MemberAction::Reconnect => Some("Reconnect"),
        MemberAction::Restart => Some("Restart"),
    };
    v_flex()
        .gap_0p5()
        .pl_3()
        .py_1()
        .border_l_1()
        .border_color(border)
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(div().flex_1().truncate().text_sm().child(member.name.clone()))
                .child(div().text_xs().text_color(muted).child(member.status.clone()))
                .when_some(button, |this, label| {
                    this.child(Button::new(id).xsmall().label(label).on_click(move |_, _, app| {
                        let file_id = file_id.clone();
                        view.update(app, |this, cx| match action {
                            MemberAction::Restart => this.restart_remote_command(&file_id, cx),
                            _ => this.reconnect_member(&file_id, cx),
                        });
                    }))
                }),
        )
        .child(
            h_flex()
                .gap_3()
                .text_xs()
                .text_color(muted)
                .child(member.count.clone())
                .child(match &member.last_line {
                    Some(at) => format!("Last line {at}"),
                    None => "No line yet".to_string(),
                })
                .when_some(member.exit.clone(), |this, exit| this.child(exit)),
        )
        .when_some(member.run_command.clone(), |this, command| {
            this.child(div().text_xs().text_color(muted).truncate().child(format!("Run: {command}")))
        })
        .when_some(member.last_error.clone(), |this, error| {
            this.child(div().text_xs().text_color(danger).child(format!("Last error {error}")))
        })
        .into_any_element()
}

fn server_block(server: &ServerDetails, view: &Entity<AppView>, app: &App) -> AnyElement {
    let theme = app.theme();
    let (muted, danger, border) = (theme.muted_foreground, theme.danger, theme.border);
    let server_id = server.server_id.clone();
    let reconnect_view = view.clone();
    let connected = server.connected_since.clone();
    let mut block = v_flex()
        .gap_1()
        .pb_2()
        .border_b_1()
        .border_color(border)
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child(server.name.clone()))
                .child(div().flex_1().truncate().text_xs().text_color(muted).child(server.address.clone()))
                .child(
                    Button::new(SharedString::from(format!("reconnect-server-{}", server.server_id)))
                        .xsmall()
                        .label("Reconnect")
                        .on_click(move |_, _, app| {
                            let id = server_id.clone();
                            reconnect_view.update(app, |this, cx| this.reconnect_server_members(&id, cx));
                        }),
                ),
        )
        .when_some(server.via.clone(), |this, via| this.child(div().text_xs().text_color(muted).child(via)))
        .child(fact(
            "State",
            match connected {
                Some(since) => format!("{} since {since}", server.state),
                None => server.state.clone(),
            },
            muted,
        ))
        .child(fact("Latency", server.latency.clone(), muted))
        .child(fact("Reconnects", server.reconnects.to_string(), muted))
        .child(fact("Keepalive", server.keepalive.clone(), muted));
    if let Some(error) = &server.last_error {
        block = block.child(h_flex().gap_2().text_xs().text_color(danger).child(format!("Last error {error}")));
    }
    for member in &server.members {
        block = block.child(member_block(member, &server.server_id, view, app));
    }
    block.into_any_element()
}

/// The content of the popover: each Server of the Log Space and its members.
pub(super) fn connection_details(snapshot: &ConnectionSnapshot, view: &Entity<AppView>, app: &App) -> AnyElement {
    let mut list = v_flex()
        .id("connection-details")
        .w(px(520.0))
        .max_h(px(480.0))
        .overflow_y_scroll()
        .gap_3()
        .p_1();
    for server in &snapshot.servers {
        list = list.child(server_block(server, view, app));
    }
    list.into_any_element()
}
