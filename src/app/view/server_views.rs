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
        let body = if dialog.form.is_some() {
            self.render_server_form(cx)
        } else {
            let mut list = v_flex().id("servers-list").gap_1().max_h(px(320.0)).overflow_y_scroll();
            if self.remote.servers.servers.is_empty() {
                list = list.child(div().text_sm().text_color(muted).child("No Servers yet."));
            }
            for server in &self.remote.servers.servers {
                let id = server.id.clone();
                let (edit_id, del_id, forget_id) = (id.clone(), id.clone(), id.clone());
                let jump = server
                    .jump_server_id
                    .as_deref()
                    .and_then(|j| self.remote.servers.get(j))
                    .map(|j| format!(" via {}", j.name))
                    .unwrap_or_default();
                list = list.child(
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
                                    "{}@{}:{}{}",
                                    server.user, server.host, server.port, jump
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
                                .on_click(cx.listener(move |this, _, _, cx| this.delete_server_from_dialog(&del_id, cx))),
                        ),
                );
            }
            v_flex()
                .gap_3()
                .child(list)
                .when_some(error.clone(), |this, e| this.child(div().text_xs().text_color(danger).child(e)))
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            Button::new("server-add")
                                .small()
                                .label("Add Server")
                                .on_click(cx.listener(|this, _, _, cx| this.new_server_form(cx))),
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
                    .w(px(560.0))
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
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let Some(fields) = form.fields.as_ref() else { return div().into_any_element() };
        let (use_key, remember, current_jump, editing) = (form.use_key, form.remember, form.jump.clone(), form.id.clone());
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
        let jump_errors: Vec<String> =
            field_errors.iter().filter(|(f, _)| *f == ServerField::JumpServer).map(|(_, m)| m.clone()).collect();

        // Jump Server: none, or one of the other Servers.
        let others: Vec<(String, String)> = self
            .remote
            .servers
            .servers
            .iter()
            .filter(|s| Some(&s.id) != editing.as_ref())
            .map(|s| (s.id.clone(), s.name.clone()))
            .collect();
        let mut jump_choices = h_flex().gap_1().flex_wrap();
        let none_selected = current_jump.is_none();
        jump_choices = jump_choices.child(
            Button::new("jump-none")
                .xsmall()
                .label("None")
                .selected(none_selected)
                .on_click(cx.listener(|this, _, _, cx| this.set_server_jump(None, cx))),
        );
        jump_choices = jump_choices.child(
            Button::new("jump-new")
                .xsmall()
                .label("New…")
                .selected(new_jump.is_some())
                .on_click(cx.listener(|this, _, _, cx| this.toggle_new_jump(cx))),
        );
        for (id, name) in others {
            let selected = current_jump.as_deref() == Some(id.as_str());
            let pick = id.clone();
            jump_choices = jump_choices.child(
                Button::new(SharedString::from(format!("jump-{id}")))
                    .xsmall()
                    .label(name)
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_server_jump(Some(pick.clone()), cx))),
            );
        }

        v_flex()
            .gap_2()
            .child(row("Name", &fields.name, ServerField::Name))
            .child(row("Address", &fields.host, ServerField::Host))
            .child(row("Port", &fields.port, ServerField::Port))
            .child(row("User", &fields.user, ServerField::User))
            .child(row("Keepalive (s)", &fields.keepalive_secs, ServerField::KeepaliveInterval))
            .child(row("Missed allowed", &fields.keepalive_missed, ServerField::KeepaliveMissed))
            .child(
                h_flex().gap_2().items_center().child(label("Login")).child(
                    ButtonGroup::new("server-auth")
                        .small()
                        .child(Button::new("auth-key").label("Private key").selected(use_key))
                        .child(Button::new("auth-password").label("Password").selected(!use_key))
                        .on_click(cx.listener(move |this, ix: &Vec<usize>, _, cx| {
                            let want_key = ix.first() == Some(&0);
                            let is_key = this
                                .server_dialog
                                .as_ref()
                                .and_then(|d| d.form.as_ref())
                                .is_some_and(|f| f.use_key);
                            if want_key != is_key {
                                this.toggle_server_auth(cx);
                            }
                        })),
                ),
            )
            .when(use_key, |this| {
                this.child(h_flex().gap_2().items_center().child(label("Key file")).child(div().flex_1().child(Input::new(&fields.key_path))))
            })
            .child(
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
            .child(h_flex().gap_2().items_start().child(label("Jump Server")).child(div().flex_1().child(jump_choices)))
            .children(jump_errors.into_iter().map(field_error))
            .when_some(self.render_new_jump_form(new_jump, cx), |this, block| this.child(block))
            .when_some(error, |this, e| this.child(div().text_xs().text_color(danger).child(e)))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
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
        let theme = cx.theme();
        let (bg, border, muted, danger) = (theme.popover, theme.border, theme.muted_foreground, theme.danger);
        let server_id = picker.server_id.clone();

        let mut servers = h_flex().gap_1().flex_wrap();
        for server in &self.remote.servers.servers {
            let id = server.id.clone();
            let selected = server_id.as_deref() == Some(id.as_str());
            servers = servers.child(
                Button::new(SharedString::from(format!("picker-server-{id}")))
                    .xsmall()
                    .label(server.name.clone())
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| this.picker_select_server(&id, cx))),
            );
        }
        servers = servers.child(
            Button::new("picker-manage")
                .xsmall()
                .ghost()
                .label("Manage…")
                .on_click(cx.listener(|this, _, _, cx| this.open_servers_dialog(cx))),
        );

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
        if picker.loading {
            list = list.child(div().p_2().text_sm().text_color(muted).child("Loading…"));
        }
        list = list.child(
            h_flex()
                .id("picker-entry-up")
                .gap_2()
                .px_2()
                .py_0p5()
                .text_sm()
                .cursor_pointer()
                .hover(|s| s.bg(border.opacity(0.4)))
                .on_click(cx.listener(|this, _, _, cx| this.picker_up(cx)))
                .child(".."),
        );
        for (ix, entry) in visible.iter().take(MAX_PICKER_ROWS).enumerate() {
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
            let name = self.remote.servers.get(item.server_id()).map(|s| s.name.clone()).unwrap_or_default();
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
                    .when(on_files, |this| this.when_some(error, |this, e| this.child(div().text_xs().text_color(danger).child(e))))
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
