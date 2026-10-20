//! UI: the Servers dialog and the remote file picker.

use super::AppView;
use crate::app::remote_picker::{MAX_PICKER_ROWS, PickerTab};
use crate::remote::browse::{SortKey, join};
use gpui::prelude::FluentBuilder as _;
use gpui::{ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_kit::assets::IconName;
use gpui_component::input::Input;
use gpui_component::Icon;
use crate::app::server_dialog::{AgentChoice, AuthKind, test_lines};
use crate::remote::ServerField;
use gpui_component::{ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui::StatefulInteractiveElement as _;

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{size:.1} {}", UNITS[unit]) }
}

fn format_time(seconds: Option<u64>) -> String {
    seconds
        .and_then(|s| chrono::DateTime::from_timestamp(s as i64, 0))
        .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

impl AppView {
    /// The Servers: the list with Edit and Delete, or the form of one.
    pub(super) fn render_servers_dialog(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(dialog) = self.server_dialog.as_ref() else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (bg, border, muted, danger) = (theme.popover, theme.border, theme.muted_foreground, theme.danger);
        let error = dialog.error.clone();
        let info = dialog.info.clone();
        let search = dialog.search.clone();
        let query = search.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
        let has_imported = self
            .remote
            .servers
            .servers
            .iter()
            .any(|s| matches!(s.origin, Some(crate::remote::Origin::SshConfig { .. })));
        let body = if dialog.import.is_some() {
            self.render_import_dialog(cx)
        } else if dialog.form.is_some() {
            self.render_server_form(cx)
        } else {
            let jump_names: std::collections::HashMap<String, String> =
                self.remote.servers.servers.iter().map(|s| (s.id.clone(), s.name.clone())).collect();
            let rows = self.render_server_list(
                "servers",
                &query,
                super::server_list::ListLayout::Rows,
                None,
                |server, cx| {
                    let id = server.id.clone();
                    let (edit_id, del_id, forget_id) = (id.clone(), id.clone(), id.clone());
                    let jump = server
                        .jump_server_id
                        .as_deref()
                        .and_then(|j| jump_names.get(j))
                        .map(|j| format!(" via {j}"))
                        .unwrap_or_default();
                    let tags = if server.tags.is_empty() { String::new() } else { format!(" \u{b7} {}", server.tags.join(", ")) };
                    h_flex()
                        .gap_2()
                        .items_center()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(border)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(0.0))
                                .child(div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child(server.name.clone()))
                                .child(div().text_xs().text_color(muted).child(format!(
                                    "{}@{}:{}{}{}",
                                    server.user, server.host, server.port, jump, tags
                                ))),
                        )
                        .child(
                            Button::new(("server-edit", id.len()))
                                .ghost()
                                .xsmall()
                                .label("Edit")
                                .on_click(cx.listener(move |this, _, _, cx| this.edit_server_form(&edit_id, cx))),
                        )
                        .child(
                            Button::new(SharedString::from(format!("server-forget-{forget_id}")))
                                .ghost()
                                .xsmall()
                                .label("Forget host key")
                                .on_click(cx.listener(move |this, _, _, cx| this.forget_server_host_key(&forget_id, cx))),
                        )
                        .child(
                            Button::new(SharedString::from(format!("server-delete-{del_id}")))
                                .ghost()
                                .xsmall()
                                .label("Delete")
                                .on_click(cx.listener(move |this, _, _, cx| this.request_delete_server(&del_id, cx))),
                        )
                        .into_any_element()
                },
                cx,
            );
            let list = v_flex().id("servers-list").gap_1().max_h(px(320.0)).overflow_y_scroll().child(rows);
            v_flex()
                .gap_3()
                .when_some(search.filter(|_| self.remote.servers.servers.len() > 1), |this, input| {
                    this.child(Input::new(&input))
                })
                .child(list)
                .when_some(error.clone(), |this, e| this.child(div().text_xs().text_color(danger).child(e)))
                .when_some(info.clone(), |this, m| this.child(div().text_xs().text_color(muted).child(m)))
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("server-add")
                                        .small()
                                        .label("Add Server")
                                        .on_click(cx.listener(|this, _, _, cx| this.new_server_form(cx))),
                                )
                                .child(
                                    Button::new("server-import")
                                        .small()
                                        .label("Import from ~/.ssh/config\u{2026}")
                                        .on_click(cx.listener(|this, _, _, cx| this.open_ssh_import(cx))),
                                )
                                .when(has_imported, |this| {
                                    this.child(
                                        Button::new("server-refresh")
                                            .small()
                                            .label("Refresh from ~/.ssh/config")
                                            .on_click(cx.listener(|this, _, _, cx| this.open_ssh_import(cx))),
                                    )
                                }),
                        )
                        .child(
                            Button::new("servers-close")
                                .primary()
                                .small()
                                .label("Done")
                                .on_click(cx.listener(|this, _, _, cx| this.close_servers_dialog(cx))),
                        ),
                )
                .into_any_element()
        };
        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(640.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child("Servers"))
                    .child(body),
            )
            .into_any_element()
    }

    fn render_server_form(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(dialog) = self.server_dialog.as_ref() else { return div().into_any_element() };
        let Some(form) = dialog.form.as_ref() else { return div().into_any_element() };
        let theme = cx.theme();
        let (muted, danger, theme_success) = (theme.muted_foreground, theme.danger, theme.success);
        let Some(fields) = form.fields.as_ref() else { return div().into_any_element() };
        let (auth, agent, remember, current_jump, editing) =
            (form.auth, form.agent, form.remember, form.jump.clone(), form.id.clone());
        let use_key = auth == AuthKind::Key;
        let has_secret = matches!(auth, AuthKind::Key | AuthKind::Password);
        let extra = form.extra.as_ref();
        let (group_input, tags_input, agent_path_input, preferred_input) = (
            extra.map(|e| e.group.clone()),
            extra.map(|e| e.tags.clone()),
            extra.map(|e| e.agent_path.clone()),
            extra.map(|e| e.preferred_key.clone()),
        );
        let test_running = form.test.as_ref().is_some_and(|t| t.running);
        let test_lines_now = form.test.as_ref().map(|t| test_lines(&t.steps)).unwrap_or_default();
        let test_error = form.test.as_ref().and_then(|t| t.error.clone());
        let test_started = form.test.is_some();
        let group_suggestions = crate::remote::server_search::group_names(&self.remote.servers.servers);
        let jump_search = form.jump_search.clone();
        let jump_query = jump_search.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
        let agent_presets = crate::remote::AgentSocket::available_presets();
        let new_jump = form.new_jump.as_ref().map(|j| (j.use_key, j.fields.is_some()));
        // Errors about the Server as a whole (a failed save); field errors
        // show under their field.
        let error = dialog.error.clone();
        let field_errors = self.visible_server_form_errors(cx);
        let error_of = |field: ServerField| field_errors.iter().find(|(f, _)| *f == field).map(|(_, m)| m.clone());
        let label = |text: &'static str| div().w(px(110.0)).flex_shrink_0().text_sm().text_color(muted).child(text);
        let field_error = |message: String| div().pl(px(118.0)).child(Self::field_error_row(message, danger));
        let row = |text: &'static str, input: &gpui::Entity<gpui_component::input::InputState>, field: ServerField| {
            let error = error_of(field);
            v_flex()
                .gap_1()
                .child(
                    h_flex().gap_2().items_center().child(label(text)).child(
                        div().flex_1().child(Input::new(input).when(error.is_some(), |i| i.border_color(danger))),
                    ),
                )
                .when_some(error, |this, e| this.child(field_error(e)))
        };
        let row_extra = |text: &'static str, input: Option<&gpui::Entity<gpui_component::input::InputState>>| {
            h_flex()
                .gap_2()
                .items_center()
                .child(label(text))
                .child(div().flex_1().when_some(input.cloned(), |this, input| this.child(Input::new(&input))))
        };
        let jump_errors: Vec<String> =
            field_errors.iter().filter(|(f, _)| *f == ServerField::JumpServer).map(|(_, m)| m.clone()).collect();

        // Jump Server: none, a new one, or one of the other Servers.
        let jump_buttons = h_flex()
            .gap_1()
            .child(
                Button::new("jump-none")
                    .xsmall()
                    .label("None")
                    .selected(current_jump.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.set_server_jump(None, cx))),
            )
            .child(
                Button::new("jump-new")
                    .xsmall()
                    .label("New\u{2026}")
                    .selected(new_jump.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_new_jump(cx))),
            );
        let chosen_jump = current_jump.clone();
        let jump_list = self.render_server_list(
            "jump",
            &jump_query,
            super::server_list::ListLayout::Wrap,
            editing.as_deref(),
            |server, cx| {
                let id = server.id.clone();
                let selected = chosen_jump.as_deref() == Some(id.as_str());
                let pick = id.clone();
                Button::new(SharedString::from(format!("jump-{id}")))
                    .xsmall()
                    .label(server.name.clone())
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_server_jump(Some(pick.clone()), cx)))
                    .into_any_element()
            },
            cx,
        );
        let jump_choices = v_flex().gap_1().child(jump_buttons).child(jump_list);

        v_flex()
            .gap_2()
            .child(row("Name", &fields.name, ServerField::Name))
            .child(row("Address", &fields.host, ServerField::Host))
            .child(row("Port", &fields.port, ServerField::Port))
            .child(row("User", &fields.user, ServerField::User))
            .child(row("Keepalive (s)", &fields.keepalive_secs, ServerField::KeepaliveInterval))
            .child(row("Missed allowed", &fields.keepalive_missed, ServerField::KeepaliveMissed))
            .child(row_extra("Server Group", group_input.as_ref()))
            .when(!group_suggestions.is_empty(), |this| {
                let mut suggestions = h_flex().gap_1().flex_wrap().pl(px(118.0));
                for (ix, name) in group_suggestions.iter().enumerate() {
                    let pick = name.clone();
                    suggestions = suggestions.child(
                        Button::new(("group-suggestion", ix))
                            .ghost()
                            .xsmall()
                            .label(name.clone())
                            .on_click(cx.listener(move |this, _, window, cx| this.set_server_group(&pick, window, cx))),
                    );
                }
                this.child(suggestions)
            })
            .child(row_extra("Tags", tags_input.as_ref()))
            .child(
                h_flex().gap_2().items_center().child(label("Login")).child(
                    ButtonGroup::new("server-auth")
                        .small()
                        .child(Button::new("auth-key").label("Private key").selected(auth == AuthKind::Key))
                        .child(Button::new("auth-password").label("Password").selected(auth == AuthKind::Password))
                        .child(Button::new("auth-agent").label("Agent").selected(auth == AuthKind::Agent))
                        .child(
                            Button::new("auth-interactive")
                                .label("Keyboard-interactive")
                                .selected(auth == AuthKind::Interactive),
                        )
                        .on_click(cx.listener(move |this, ix: &Vec<usize>, _, cx| {
                            let kind = match ix.first() {
                                Some(0) => AuthKind::Key,
                                Some(1) => AuthKind::Password,
                                Some(2) => AuthKind::Agent,
                                _ => AuthKind::Interactive,
                            };
                            this.set_server_auth(kind, cx);
                        })),
                ),
            )
            .when(use_key, |this| {
                this.child(h_flex().gap_2().items_center().child(label("Key file")).child(div().flex_1().child(Input::new(&fields.key_path))))
            })
            .when(auth == AuthKind::Agent, |this| {
                let mut sockets = h_flex().gap_1().flex_wrap();
                let choice = |id: &'static str, text: &'static str, which: AgentChoice, cx: &mut gpui::Context<Self>| {
                    Button::new(id)
                        .xsmall()
                        .label(text)
                        .selected(agent == which)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_agent_choice(which, cx)))
                };
                sockets = sockets.child(choice("agent-system", "System", AgentChoice::System, cx));
                if agent_presets.contains(&crate::remote::AgentSocket::OnePassword) || agent == AgentChoice::OnePassword {
                    sockets = sockets.child(choice("agent-1password", "1Password", AgentChoice::OnePassword, cx));
                }
                if agent_presets.contains(&crate::remote::AgentSocket::Secretive) || agent == AgentChoice::Secretive {
                    sockets = sockets.child(choice("agent-secretive", "Secretive", AgentChoice::Secretive, cx));
                }
                sockets = sockets.child(choice("agent-path", "Path\u{2026}", AgentChoice::Path, cx));
                let socket_now = match agent {
                    AgentChoice::System => crate::remote::AgentSocket::System,
                    AgentChoice::OnePassword => crate::remote::AgentSocket::OnePassword,
                    AgentChoice::Secretive => crate::remote::AgentSocket::Secretive,
                    AgentChoice::Path => crate::remote::AgentSocket::Path(String::new()),
                };
                this.child(h_flex().gap_2().items_start().child(label("Agent socket")).child(div().flex_1().child(sockets)))
                    .when(agent != AgentChoice::Path, |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .child(label(""))
                                .child(div().text_xs().text_color(muted).child(socket_now.describe())),
                        )
                    })
                    .when_some(agent_path_input.filter(|_| agent == AgentChoice::Path), |this, input| {
                        this.child(h_flex().gap_2().items_center().child(label("Socket path")).child(div().flex_1().child(Input::new(&input))))
                    })
                    .when_some(preferred_input, |this, input| {
                        this.child(h_flex().gap_2().items_center().child(label("Preferred key")).child(div().flex_1().child(Input::new(&input))))
                    })
            })
            .when(has_secret, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(label(if use_key { "Passphrase" } else { "Password" }))
                        .child(div().flex_1().child(Input::new(&fields.secret))),
                )
                .child(
                    h_flex().gap_2().child(label("")).child(
                        Checkbox::new("server-remember")
                            .label(crate::ui::platform::REMEMBER_SECRET)
                            .checked(remember)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_server_remember(cx))),
                    ),
                )
            })
            .child(
                h_flex().gap_2().items_start().child(label("Jump Server")).child(
                    v_flex()
                        .flex_1()
                        .gap_1()
                        .when_some(jump_search.filter(|_| self.remote.servers.servers.len() > 4), |this, input| {
                            this.child(Input::new(&input))
                        })
                        .child(jump_choices),
                ),
            )
            .children(jump_errors.into_iter().map(field_error))
            .when_some(self.render_new_jump_form(new_jump, cx), |this, block| this.child(block))
            .when_some(error, |this, e| this.child(div().text_xs().text_color(danger).child(e)))
            .when(test_started, |this| {
                let success = theme_success;
                let mut results = v_flex().gap_0p5().pl(px(118.0));
                for (line, ok) in test_lines_now {
                    results = results.child(div().text_xs().text_color(if ok { success } else { danger }).child(line));
                }
                if test_running {
                    results = results.child(div().text_xs().text_color(muted).child("Testing\u{2026}"));
                }
                if let Some(e) = test_error {
                    results = results.child(div().text_xs().text_color(danger).child(e));
                }
                this.child(results)
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(if test_running {
                        Button::new("server-test")
                            .label("Cancel test")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_server_test(cx)))
                    } else {
                        Button::new("server-test")
                            .label("Test connection")
                            .on_click(cx.listener(|this, _, _, cx| this.test_server_connection(cx)))
                    })
                    .child(
                        Button::new("server-cancel")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_server_form(cx))),
                    )
                    .child(
                        Button::new("server-save")
                            .primary()
                            .label("Save")
                            .on_click(cx.listener(|this, _, _, cx| this.save_server_form(cx))),
                    ),
            )
            .into_any_element()
    }

    /// "Import from ~/.ssh/config": the hosts to take, what changed in the
    /// ones taken before, and what was left out.
    fn render_import_dialog(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(import) = self.server_dialog.as_ref().and_then(|d| d.import.as_ref()) else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (muted, danger, border) = (theme.muted_foreground, theme.danger, theme.border);
        if !import.config_found {
            return v_flex()
                .gap_3()
                .child(div().text_sm().child("No ~/.ssh/config found"))
                .child(
                    h_flex().justify_end().child(
                        Button::new("import-back")
                            .label("Back")
                            .on_click(cx.listener(|this, _, _, cx| this.close_ssh_import(cx))),
                    ),
                )
                .into_any_element();
        }
        let mut rows = v_flex().id("import-rows").gap_1().max_h(px(300.0)).overflow_y_scroll();
        for (ix, candidate) in import.new.iter().enumerate() {
            let alias = candidate.alias.clone();
            let ticked = import.ticked.contains(&alias);
            let disabled = candidate.error.is_some();
            let summary = format!(
                "{}@{}:{} \u{b7} {}{}",
                candidate.server.user,
                candidate.server.host,
                candidate.server.port,
                auth_label(&candidate.server.auth),
                if candidate.hops.is_empty() {
                    String::new()
                } else {
                    format!(" \u{b7} via {}", candidate.hops.iter().map(|h| h.describe()).collect::<Vec<_>>().join(" \u{2192} "))
                }
            );
            let mut notes: Vec<String> = candidate.warnings.clone();
            notes.extend(candidate.error.clone());
            let name_input = import.name_inputs.get(&alias).cloned();
            rows = rows.child(
                v_flex()
                    .gap_0p5()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child({
                                let toggle = alias.clone();
                                Checkbox::new(("import-row", ix))
                                    .checked(ticked)
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_import_row(&toggle, cx)))
                            })
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .child(div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child(alias.clone()))
                                    .child(div().text_xs().text_color(muted).child(summary)),
                            )
                            .when_some(name_input, |this, input| this.child(div().w(px(160.0)).child(Input::new(&input)))),
                    )
                    .children(notes.into_iter().map(|n| div().pl(px(26.0)).text_xs().text_color(danger).child(n))),
            );
        }
        let mut changes = v_flex().gap_1();
        let mut any_changes = false;
        let mut gone: Vec<String> = Vec::new();
        for row in &import.known {
            match row {
                crate::remote::ssh_config::RefreshRow::Changed { server_id, name, changes: list } => {
                    for (ix, change) in list.iter().enumerate() {
                        any_changes = true;
                        let (id, field) = (server_id.clone(), change.field);
                        let ticked = import.ticked_changes.contains(&(id.clone(), field));
                        changes = changes.child(
                            Checkbox::new(SharedString::from(format!("import-change-{server_id}-{ix}")))
                                .label(format!("{name}: {}", crate::app::ssh_import::change_text(change)))
                                .checked(ticked)
                                .on_click(cx.listener(move |this, _, _, cx| this.toggle_import_change(&id, field, cx))),
                        );
                    }
                }
                crate::remote::ssh_config::RefreshRow::Gone { name, .. } => {
                    gone.push(format!("{name}: No longer in ~/.ssh/config"))
                }
                crate::remote::ssh_config::RefreshRow::Same { .. } => {}
            }
        }
        let skipped = import.skipped.clone();
        let group_input = import.group_input.clone();
        let has_work = import.has_work();
        let empty = import.new.is_empty() && !any_changes && gone.is_empty();
        v_flex()
            .gap_3()
            .when(!import.new.is_empty(), |this| {
                this.child(div().text_xs().text_color(muted).child("Hosts in ~/.ssh/config")).child(rows)
            })
            .when(any_changes, |this| {
                this.child(div().text_xs().text_color(muted).child("Changed since they were imported"))
                    .child(changes)
            })
            .when(!gone.is_empty(), |this| {
                this.children(gone.into_iter().map(|g| div().text_xs().text_color(muted).child(g)))
            })
            .when(empty, |this| this.child(div().text_sm().text_color(muted).child("Everything in ~/.ssh/config is already imported")))
            .when(!skipped.is_empty(), |this| {
                this.child(div().text_xs().text_color(muted).child("Skipped")).child(
                    v_flex().gap_0p5().children(
                        skipped.into_iter().map(|s| div().text_xs().text_color(muted).child(format!("{}: {}", s.what, s.why))),
                    ),
                )
            })
            .when_some(group_input.filter(|_| !import.new.is_empty()), |this, input| {
                this.child(h_flex().gap_2().items_center().child(div().w(px(110.0)).text_sm().text_color(muted).child("Server Group")).child(div().flex_1().child(Input::new(&input))))
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("import-cancel")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.close_ssh_import(cx))),
                    )
                    .child(
                        Button::new("import-apply")
                            .primary()
                            .label("Import")
                            .disabled(!has_work)
                            .on_click(cx.listener(|this, _, _, cx| this.apply_ssh_import(cx))),
                    ),
            )
            .into_any_element()
    }

    /// The Quick Connect field, the Servers it made that are not saved yet,
    /// and the small sign-in form when the agent was refused.
    fn render_quick_connect(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let Some(picker) = self.remote.picker.as_ref() else { return div().into_any_element() };
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let selected = picker.server_id.clone();
        let mut block = v_flex().gap_1().child(
            h_flex()
                .gap_2()
                .items_center()
                .child(div().text_xs().text_color(muted).child("Quick Connect"))
                .child(div().flex_1().when_some(picker.quick_input.clone(), |this, input| this.child(Input::new(&input)))),
        );
        if !picker.quick_servers.is_empty() {
            let mut made = h_flex().gap_1().flex_wrap().items_center().child(
                div().text_xs().text_color(muted).child("New, saved when a file is added:"),
            );
            for server in &picker.quick_servers {
                let id = server.id.clone();
                let pick = id.clone();
                made = made.child(
                    Button::new(SharedString::from(format!("picker-quick-{id}")))
                        .xsmall()
                        .label(server.name.clone())
                        .selected(selected.as_deref() == Some(id.as_str()))
                        .on_click(cx.listener(move |this, _, _, cx| this.picker_select_server(&pick, cx))),
                );
            }
            block = block.child(made);
        }
        if let Some(login) = picker.quick_login.as_ref() {
            let (use_key, remember) = (login.use_key, login.remember);
            let label = |text: &'static str| div().w(px(90.0)).flex_shrink_0().text_sm().text_color(muted).child(text);
            block = block.child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .child(div().text_sm().child(format!(
                        "Sign in to {} with password or key file",
                        login.server_name
                    )))
                    .child(
                        h_flex().gap_1().child(
                            ButtonGroup::new("quick-login-kind")
                                .xsmall()
                                .child(Button::new("quick-login-password").label("Password").selected(!use_key))
                                .child(Button::new("quick-login-key").label("Key file").selected(use_key))
                                .on_click(cx.listener(|this, ix: &Vec<usize>, _, cx| {
                                    this.quick_login_set_key(ix.first() == Some(&1), cx)
                                })),
                        ),
                    )
                    .when(use_key, |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(label("Key file"))
                                .child(div().flex_1().when_some(login.key_path.clone(), |t, i| t.child(Input::new(&i)))),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(label(if use_key { "Passphrase" } else { "Password" }))
                            .child(div().flex_1().when_some(login.secret.clone(), |t, i| t.child(Input::new(&i)))),
                    )
                    .child(
                        Checkbox::new("quick-login-remember")
                            .label(crate::ui::platform::REMEMBER_SECRET)
                            .checked(remember)
                            .on_click(cx.listener(|this, _, _, cx| this.quick_login_toggle_remember(cx))),
                    )
                    .when_some(login.error.clone(), |this, e| this.child(div().text_xs().text_color(danger).child(e)))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("quick-login-cancel")
                                    .xsmall()
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.quick_login_cancel(cx))),
                            )
                            .child(
                                Button::new("quick-login-submit")
                                    .xsmall()
                                    .primary()
                                    .label("Sign in")
                                    .on_click(cx.listener(|this, _, _, cx| this.quick_login_submit(cx))),
                            ),
                    ),
            );
        }
        block.into_any_element()
    }

    /// The Jump Server typed together with the Server, indented under it.
    fn render_new_jump_form(
        &mut self,
        state: Option<(bool, bool)>,
        cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (use_key, ready) = state?;
        if !ready {
            return None;
        }
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let jump = self.server_dialog.as_ref()?.form.as_ref()?.new_jump.as_ref()?;
        let fields = jump.fields.as_ref()?;
        let label = |text: &'static str| div().w(px(110.0)).text_sm().text_color(muted).child(text);
        let row = |text: &'static str, input: &gpui::Entity<gpui_component::input::InputState>| {
            h_flex().gap_2().items_center().child(label(text)).child(div().flex_1().child(Input::new(input)))
        };
        Some(
            v_flex()
                .gap_2()
                .ml(px(24.0))
                .pl_3()
                .border_l_2()
                .border_color(border)
                .child(div().text_xs().text_color(muted).child("New Jump Server: the Server above is reached through it"))
                .child(row("Name", &fields.name))
                .child(row("Address", &fields.host))
                .child(row("Port", &fields.port))
                .child(row("User", &fields.user))
                .child(
                    h_flex().gap_2().items_center().child(label("Login")).child(
                        ButtonGroup::new("jump-auth")
                            .small()
                            .child(Button::new("jump-auth-key").label("Private key").selected(use_key))
                            .child(Button::new("jump-auth-password").label("Password").selected(!use_key))
                            .on_click(cx.listener(move |this, ix: &Vec<usize>, _, cx| {
                                let want_key = ix.first() == Some(&0);
                                let is_key = this
                                    .server_dialog
                                    .as_ref()
                                    .and_then(|d| d.form.as_ref())
                                    .and_then(|f| f.new_jump.as_ref())
                                    .is_some_and(|j| j.use_key);
                                if want_key != is_key {
                                    this.toggle_new_jump_auth(cx);
                                }
                            })),
                    ),
                )
                .when(use_key, |this| this.child(row("Key file", &fields.key_path)))
                .child(row(if use_key { "Passphrase" } else { "Password" }, &fields.secret))
                .into_any_element(),
        )
    }

    /// The picker of Remote Files: Server buttons, a folder listing with
    /// checkboxes, a path or glob field, and the files chosen so far.
    pub(super) fn render_remote_picker(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let command_body = match self.remote.picker.as_ref().map(|p| p.tab) {
            Some(PickerTab::Command) => Some(self.render_command_tab(cx)),
            _ => None,
        };
        let Some(picker) = self.remote.picker.as_ref() else { return div().into_any_element() };
        let server_id = picker.server_id.clone();

        let server_search = picker.server_search.clone();
        let query = server_search.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
        let many_servers = self.remote.servers.servers.len() > 6;
        let selected_server = server_id.clone();
        let chooser = self.render_server_list(
            "picker-servers",
            &query,
            super::server_list::ListLayout::Wrap,
            None,
            |server, cx| {
                let id = server.id.clone();
                let selected = selected_server.as_deref() == Some(id.as_str());
                Button::new(SharedString::from(format!("picker-server-{id}")))
                    .xsmall()
                    .label(server.name.clone())
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| this.picker_select_server(&id, cx)))
                    .into_any_element()
            },
            cx,
        );
        let servers = v_flex()
            .gap_1()
            .when_some(server_search.filter(|_| many_servers), |this, input| this.child(Input::new(&input)))
            .child(v_flex().id("picker-server-list").max_h(px(120.0)).overflow_y_scroll().child(chooser))
            .child(
                h_flex().child(
                    Button::new("picker-manage")
                        .xsmall()
                        .ghost()
                        .label("Manage\u{2026}")
                        .on_click(cx.listener(|this, _, _, cx| this.open_servers_dialog(cx))),
                ),
            );
        let quick = self.render_quick_connect(cx);
        // The picker is borrowed again below; take what it needs now.
        let Some(picker) = self.remote.picker.as_ref() else { return div().into_any_element() };
        let theme = cx.theme();
        let (bg, border, muted, danger) = (theme.popover, theme.border, theme.muted_foreground, theme.danger);

        let sort = picker.sort;
        let visible = picker.visible_entries();
        let hidden_rows = visible.len().saturating_sub(MAX_PICKER_ROWS);
        let arrow = |key: SortKey| {
            if sort.key == key { if sort.descending { " ▼" } else { " ▲" } } else { "" }
        };
        let header_cell = |id: &'static str, label: &'static str, key: SortKey, cx: &mut gpui::Context<Self>| {
            div()
                .id(id)
                .cursor_pointer()
                .text_xs()
                .text_color(muted)
                .on_click(cx.listener(move |this, _, _, cx| this.picker_sort_by(key, cx)))
                .child(format!("{label}{}", arrow(key)))
        };
        let header = h_flex()
            .gap_2()
            .px_2()
            .py_0p5()
            .border_b_1()
            .border_color(border)
            .child(div().flex_1().child(header_cell("picker-sort-name", "Name", SortKey::Name, cx)))
            .child(div().w(px(72.0)).child(header_cell("picker-sort-size", "Size", SortKey::Size, cx)))
            .child(div().w(px(120.0)).child(header_cell("picker-sort-modified", "Modified", SortKey::Modified, cx)));

        let mut list = v_flex().id("picker-list").h(px(240.0)).overflow_y_scroll();
        let shows_rows = picker.shows_rows();
        let list_notice = picker.list_notice();
        let resolving = match &picker.busy {
            Some(busy @ crate::app::remote_picker::PickerBusy::Resolving(_)) => Some(busy.label()),
            _ => None,
        };
        let info = picker.info.clone();
        list = list.when(shows_rows, |list| list.child(
            h_flex()
                .id("picker-entry-up")
                .gap_2()
                .px_2()
                .py_0p5()
                .text_sm()
                .cursor_pointer()
                .hover(|s| s.bg(border.opacity(0.4)))
                .on_click(cx.listener(|this, _, _, cx| this.picker_up(cx)))
                .child(Icon::new(IconName::FolderOpen).size_3().text_color(muted))
                .child(".."),
        ));
        if let Some(notice) = list_notice {
            list = list.child(div().p_2().text_sm().text_color(muted).child(notice));
        }
        let rows = if shows_rows { visible.len() } else { 0 };
        for (ix, entry) in visible.iter().take(MAX_PICKER_ROWS.min(rows)).enumerate() {
            let name = entry.name.clone();
            if entry.is_dir {
                let open = name.clone();
                list = list.child(
                    h_flex()
                        .id(ElementId::from(("picker-entry", ix)))
                        .gap_2()
                        .items_center()
                        .px_2()
                        .py_0p5()
                        .text_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(border.opacity(0.4)))
                        .on_click(cx.listener(move |this, _, _, cx| this.picker_open_dir(&open, cx)))
                        .child(Icon::new(IconName::Folder).size_3().text_color(muted))
                        .child(div().flex_1().child(format!("{name}/")))
                        .child(div().w(px(72.0)))
                        .child(div().text_xs().text_color(muted).w(px(120.0)).child(format_time(entry.modified))),
                );
            } else {
                let path = server_id.as_deref().map(|_| join(&picker.dir, &name)).unwrap_or_default();
                let staged = server_id.as_deref().is_some_and(|s| picker.is_staged(s, &path));
                let toggle = name.clone();
                list = list.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .px_2()
                        .py_0p5()
                        .text_sm()
                        .child(
                            Checkbox::new(ElementId::from(("picker-file", ix)))
                                .checked(staged)
                                .on_click(cx.listener(move |this, _, _, cx| this.picker_toggle_file(&toggle, cx))),
                        )
                        .child(Icon::new(IconName::File).size_3().text_color(muted))
                        .child(div().flex_1().child(name))
                        .child(div().text_xs().text_color(muted).w(px(72.0)).child(format_size(entry.size)))
                        .child(div().text_xs().text_color(muted).w(px(120.0)).child(format_time(entry.modified))),
                );
            }
        }
        if hidden_rows > 0 {
            list = list.child(
                div().p_2().text_xs().text_color(muted).child(format!("{hidden_rows} more: type in the filter to narrow the list")),
            );
        }
        let list = v_flex()
            .border_1()
            .border_color(border)
            .rounded_md()
            .child(header)
            .child(list);

        let mut staged = v_flex().id("picker-staged").gap_0p5().max_h(px(96.0)).overflow_y_scroll();
        for (ix, item) in picker.staged.iter().enumerate() {
            let name = self.picker_server(item.server_id()).map(|s| s.name).unwrap_or_default();
            staged = staged.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_xs()
                    .when(item.is_glob(), |this| this.child(div().text_color(muted).child("Live Glob")))
                    .child(div().flex_1().truncate().child(format!("{name}:{}", item.path())))
                    .child(
                        Button::new(("picker-unstage", ix))
                            .ghost()
                            .xsmall()
                            .label("Remove")
                            .on_click(cx.listener(move |this, _, _, cx| this.picker_unstage(ix, cx))),
                    ),
            );
        }
        let count = picker.staged.len();
        let globs = picker.staged.iter().filter(|s| s.is_glob()).count();
        let dir_label = picker.dir.clone();
        let error = picker.error.clone();
        let input = picker.input.clone();
        let filter_input = picker.filter_input.clone();
        let tab = picker.tab;
        let on_files = tab == PickerTab::Files;
        let can_add_command = !on_files
            && self.command_server().is_some()
            && !self.command_text(cx).is_empty()
            && self.command_form_error(cx).is_none();

        // Servers to add the typed path on, the selected one always ticked.
        let mut ticks = h_flex().gap_2().flex_wrap().items_center();
        if self.remote.servers.servers.len() > 1 {
            ticks = ticks.child(div().text_xs().text_color(muted).child("Add on:"));
            for server in &self.remote.servers.servers {
                let id = server.id.clone();
                let ticked = picker.is_ticked(&id);
                ticks = ticks.child(
                    Checkbox::new(SharedString::from(format!("picker-tick-{id}")))
                        .label(server.name.clone())
                        .checked(ticked)
                        .on_click(cx.listener(move |this, _, _, cx| this.picker_toggle_server(&id, cx))),
                );
            }
        }

        // Recent Paths of the selected Server.
        let recent: Vec<crate::remote::picker_state::RecentPath> = server_id
            .as_deref()
            .map(|s| self.remote.picker_state.recent(s).to_vec())
            .unwrap_or_default();
        let mut recent_block = v_flex().gap_0p5();
        if !recent.is_empty() {
            recent_block = recent_block.child(
                h_flex()
                    .items_center()
                    .child(div().flex_1().text_xs().text_color(muted).child("Recent paths"))
                    .child(
                        Button::new("picker-recent-clear")
                            .ghost()
                            .xsmall()
                            .label("Clear")
                            .on_click(cx.listener(|this, _, _, cx| this.picker_clear_recent(cx))),
                    ),
            );
            for (ix, r) in recent.iter().enumerate() {
                recent_block = recent_block.child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .text_xs()
                        .child(
                            div()
                                .id(ElementId::from(("picker-recent", ix)))
                                .flex_1()
                                .truncate()
                                .cursor_pointer()
                                .hover(|s| s.bg(border.opacity(0.4)))
                                .on_click(cx.listener(move |this, _, _, cx| this.picker_add_recent(ix, cx)))
                                .child(r.path.clone()),
                        )
                        .child(
                            Button::new(("picker-recent-remove", ix))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .on_click(cx.listener(move |this, _, _, cx| this.picker_remove_recent(ix, cx))),
                        ),
                );
            }
        }

        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(640.0))
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
                            .child(if on_files { "Add remote files" } else { "Add remote command" }),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("picker-tab-files")
                                    .xsmall()
                                    .label("Files")
                                    .selected(on_files)
                                    .on_click(cx.listener(|this, _, _, cx| this.picker_set_tab(PickerTab::Files, cx))),
                            )
                            .child(
                                Button::new("picker-tab-command")
                                    .xsmall()
                                    .label("Command")
                                    .selected(!on_files)
                                    .on_click(cx.listener(|this, _, _, cx| this.picker_set_tab(PickerTab::Command, cx))),
                            ),
                    )
                    .when(on_files, |this| this.child(quick))
                    .child(servers)
                    .when_some(command_body, |this, body| this.child(body))
                    .when(on_files, |this| this
                        .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("picker-up")
                                    .xsmall()
                                    .label("Up")
                                    .on_click(cx.listener(|this, _, _, cx| this.picker_up(cx))),
                            )
                            .child(
                                Button::new("picker-refresh")
                                    .xsmall()
                                    .label("Refresh")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let dir = this.remote.picker.as_ref().map(|p| p.dir.clone());
                                        this.picker_load(dir, cx);
                                    })),
                            )
                            .child(
                                Button::new("picker-home")
                                    .xsmall()
                                    .label("Home")
                                    .on_click(cx.listener(|this, _, _, cx| this.picker_go_home(cx))),
                            )
                            .child(
                                Button::new("picker-var-log")
                                    .xsmall()
                                    .label("/var/log")
                                    .on_click(cx.listener(|this, _, _, cx| this.picker_go_var_log(cx))),
                            )
                            .child(div().flex_1().truncate().text_sm().text_color(muted).child(dir_label)),
                    )
                    .child(div().child(filter_input.map(|i| Input::new(&i)).map(|i| i.into_any_element()).unwrap_or_else(|| div().into_any_element())))
                    .child(list)
                    .child(recent_block)
                    .child(ticks)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().when_some(input, |this, input| this.child(Input::new(&input))))
                            .child(
                                Button::new("picker-add")
                                    .small()
                                    .label("Add path")
                                    .on_click(cx.listener(|this, _, _, cx| this.picker_add_typed(cx))),
                            ),
                    ))
                    .when(on_files, |this| {
                        this.when_some(resolving, |this, m| this.child(div().text_xs().text_color(muted).child(m)))
                            .when_some(info, |this, m| this.child(div().text_xs().text_color(muted).child(m)))
                            .when_some(error, |this, e| this.child(div().text_xs().text_color(danger).child(e)))
                    })
                    .when(on_files && count > 0, |this| this.child(staged))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("picker-cancel")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| this.close_remote_picker(cx))),
                            )
                            .when(on_files, |this| this.child(
                                Button::new("picker-done")
                                    .primary()
                                    .label(match (count, globs) {
                                        (1, 0) => "Add 1 file".to_string(),
                                        (n, 0) => format!("Add {n} files"),
                                        (n, g) => format!("Add {n} items ({g} Live Glob{})", if g == 1 { "" } else { "s" }),
                                    })
                                    .disabled(count == 0)
                                    .on_click(cx.listener(|this, _, _, cx| this.picker_confirm(cx))),
                            ))
                            .when(!on_files, |this| this.child(
                                Button::new("command-add")
                                    .primary()
                                    .label("Add")
                                    .disabled(!can_add_command)
                                    .on_click(cx.listener(|this, _, _, cx| this.command_add(cx))),
                            )),
                    ),
            )
            .into_any_element()
    }
}

impl AppView {
    /// The download: progress with Cancel, then the offer to open the files.
    pub(super) fn render_download(&mut self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        use std::sync::atomic::Ordering;
        let Some(download) = self.remote.download.as_ref() else { return div().into_any_element() };
        let theme = cx.theme();
        let (bg, border, muted, danger) = (theme.popover, theme.border, theme.muted_foreground, theme.danger);
        let total_files = download.names.len();
        let done = download.progress.files_done.load(Ordering::Relaxed);
        let bytes = download.progress.bytes.load(Ordering::Relaxed);
        let total = download.progress.total.load(Ordering::Relaxed);
        let dir = download.dir.to_string_lossy().to_string();
        let current = download.names.get(done).cloned().unwrap_or_default();
        let body = match &download.outcome {
            None => v_flex()
                .gap_2()
                .child(div().text_sm().child(format!(
                    "Downloading {} of {total_files}: {current}",
                    (done + 1).min(total_files)
                )))
                .child(div().text_xs().text_color(muted).child(format!(
                    "{} of {}",
                    format_size(bytes),
                    format_size(total)
                )))
                .child(
                    h_flex().justify_end().child(
                        Button::new("download-cancel")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_download(cx))),
                    ),
                )
                .into_any_element(),
            Some(Ok(paths)) => v_flex()
                .gap_2()
                .child(div().text_sm().child(format!("Downloaded {} file(s) to {dir}.", paths.len())))
                .child(div().text_sm().child("Open in a new Imported Log Space?"))
                .child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("download-not-now")
                                .label("Not now")
                                .on_click(cx.listener(|this, _, _, cx| this.finish_download(false, cx))),
                        )
                        .child(
                            Button::new("download-open")
                                .primary()
                                .label("Open")
                                .on_click(cx.listener(|this, _, _, cx| this.finish_download(true, cx))),
                        ),
                )
                .into_any_element(),
            Some(Err(message)) => v_flex()
                .gap_2()
                .child(div().text_sm().text_color(danger).child(message.clone()))
                .child(
                    h_flex().justify_end().child(
                        Button::new("download-close")
                            .label("Close")
                            .on_click(cx.listener(|this, _, _, cx| this.finish_download(false, cx))),
                    ),
                )
                .into_any_element(),
        };
        Self::modal_backdrop()
            .child(
                v_flex()
                    .w(px(440.0))
                    .gap_3()
                    .p_4()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_lg()
                    .shadow_2xl()
                    .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child("Download"))
                    .child(body),
            )
            .into_any_element()
    }
}

/// How a Server signs in, in a few words.
fn auth_label(auth: &crate::remote::AuthMethod) -> String {
    match auth {
        crate::remote::AuthMethod::Key { path } => format!("key {path}"),
        crate::remote::AuthMethod::Password => "password".into(),
        crate::remote::AuthMethod::Agent { socket, .. } => format!("agent {}", socket.describe()),
        crate::remote::AuthMethod::KeyboardInteractive => "keyboard-interactive".into(),
    }
}
