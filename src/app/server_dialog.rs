//! Logic: the Servers dialog: the list, and the form that adds or edits one.

use super::AppView;
use crate::remote::{AuthMethod, Server, ServerError, ServerField, parse_port};
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use std::collections::HashSet;

/// The fields of the form, created while rendering where a window exists.
pub struct ServerFields {
    pub name: Entity<InputState>,
    pub host: Entity<InputState>,
    pub port: Entity<InputState>,
    pub user: Entity<InputState>,
    pub key_path: Entity<InputState>,
    pub secret: Entity<InputState>,
    pub keepalive_secs: Entity<InputState>,
    pub keepalive_missed: Entity<InputState>,
}

/// A Jump Server typed in the same form as the Server that goes through it.
pub struct NewJump {
    pub use_key: bool,
    pub fields: Option<ServerFields>,
}

pub struct ServerForm {
    /// `None` for a new Server.
    pub id: Option<String>,
    pub initial: Server,
    pub use_key: bool,
    pub jump: Option<String>,
    pub remember: bool,
    pub fields: Option<ServerFields>,
    /// A Jump Server being made together with this Server.
    pub new_jump: Option<NewJump>,
    /// Fields the user has left at least once: their errors show.
    pub touched: HashSet<ServerField>,
    /// Save was pressed: every field's error shows.
    pub save_attempted: bool,
    /// Blur listeners of the fields.
    pub subscriptions: Vec<Subscription>,
}

#[derive(Default)]
pub struct ServerDialog {
    pub form: Option<ServerForm>,
    pub error: Option<String>,
}

impl AppView {
    pub fn open_servers_dialog(&mut self, cx: &mut Context<Self>) {
        self.is_palette_open = false;
        self.server_dialog = Some(ServerDialog::default());
        cx.notify();
    }

    pub fn close_servers_dialog(&mut self, cx: &mut Context<Self>) {
        self.server_dialog = None;
        cx.notify();
    }

    pub fn new_server_form(&mut self, cx: &mut Context<Self>) {
        let initial = Server::new("", "", "", AuthMethod::Key { path: "~/.ssh/id_ed25519".into() });
        self.set_server_form(None, initial, cx);
    }

    pub fn edit_server_form(&mut self, server_id: &str, cx: &mut Context<Self>) {
        if let Some(server) = self.remote.servers.get(server_id).cloned() {
            self.set_server_form(Some(server.id.clone()), server, cx);
        }
    }

    fn set_server_form(&mut self, id: Option<String>, initial: Server, cx: &mut Context<Self>) {
        let dialog = self.server_dialog.get_or_insert_with(ServerDialog::default);
        dialog.error = None;
        dialog.form = Some(ServerForm {
            id,
            use_key: matches!(initial.auth, AuthMethod::Key { .. }),
            jump: initial.jump_server_id.clone(),
            remember: initial.remember_secret,
            initial,
            fields: None,
            new_jump: None,
            touched: HashSet::new(),
            save_attempted: false,
            subscriptions: Vec::new(),
        });
        cx.notify();
    }

    pub fn cancel_server_form(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.form = None;
            dialog.error = None;
        }
        cx.notify();
    }

    pub fn toggle_server_auth(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            form.use_key = !form.use_key;
        }
        cx.notify();
    }

    pub fn set_server_jump(&mut self, jump: Option<String>, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            form.jump = jump;
            form.new_jump = None;
        }
        cx.notify();
    }

    /// Type a new Jump Server in this form, or stop doing so.
    pub fn toggle_new_jump(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            if form.new_jump.is_some() {
                form.new_jump = None;
            } else {
                form.jump = None;
                form.new_jump = Some(NewJump { use_key: true, fields: None });
            }
        }
        cx.notify();
    }

    pub fn toggle_new_jump_auth(&mut self, cx: &mut Context<Self>) {
        if let Some(jump) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()).and_then(|f| f.new_jump.as_mut()) {
            jump.use_key = !jump.use_key;
        }
        cx.notify();
    }

    pub fn toggle_server_remember(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            form.remember = !form.remember;
        }
        cx.notify();
    }

    /// Create the fields of a freshly opened form.
    pub fn sync_server_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let make_jump_fields = self
            .server_dialog
            .as_ref()
            .and_then(|d| d.form.as_ref())
            .is_some_and(|f| f.new_jump.as_ref().is_some_and(|j| j.fields.is_none()));
        if make_jump_fields {
            let field = |placeholder: &'static str, value: &str, window: &mut Window, cx: &mut Context<Self>| {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                if !value.is_empty() {
                    input.update(cx, |state, cx| state.set_value(value.to_string(), window, cx));
                }
                input
            };
            let secret = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Password or key passphrase"));
            let fields = ServerFields {
                name: field("bastion", "", window, cx),
                host: field("bastion.example.com", "", window, cx),
                port: field("22", "22", window, cx),
                user: field("deploy", "", window, cx),
                key_path: field("~/.ssh/id_ed25519", "~/.ssh/id_ed25519", window, cx),
                secret,
                keepalive_secs: field("30", "30", window, cx),
                keepalive_missed: field("3", "3", window, cx),
            };
            let subs = [&fields.name, &fields.host, &fields.port, &fields.user]
                .into_iter()
                .map(|input| self.touch_on_blur(input, ServerField::JumpServer, window, cx))
                .collect::<Vec<_>>();
            if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
                form.subscriptions.extend(subs);
                if let Some(jump) = form.new_jump.as_mut() {
                    jump.fields = Some(fields);
                }
            }
        }
        let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) else { return };
        if form.fields.is_some() {
            return;
        }
        let initial = form.initial.clone();
        let key_path = match &initial.auth {
            AuthMethod::Key { path } => path.clone(),
            AuthMethod::Password => "~/.ssh/id_ed25519".to_string(),
        };
        let field = |placeholder: &'static str, value: String, window: &mut Window, cx: &mut Context<Self>| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            input
        };
        let secret = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Password or key passphrase"));
        let fields = ServerFields {
            name: field("prod-web1", initial.name.clone(), window, cx),
            host: field("10.0.0.5 or host name", initial.host.clone(), window, cx),
            port: field("22", initial.port.to_string(), window, cx),
            user: field("deploy", initial.user.clone(), window, cx),
            key_path: field("~/.ssh/id_ed25519", key_path, window, cx),
            secret,
            keepalive_secs: field("30", initial.keepalive_interval_secs.to_string(), window, cx),
            keepalive_missed: field("3", initial.keepalive_max_missed.to_string(), window, cx),
        };
        fields.name.update(cx, |state, cx| state.focus(window, cx));
        let subs = [
            (&fields.name, ServerField::Name),
            (&fields.host, ServerField::Host),
            (&fields.port, ServerField::Port),
            (&fields.user, ServerField::User),
            (&fields.keepalive_secs, ServerField::KeepaliveInterval),
            (&fields.keepalive_missed, ServerField::KeepaliveMissed),
        ]
        .into_iter()
        .map(|(input, field)| self.touch_on_blur(input, field, window, cx))
        .collect::<Vec<_>>();
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            form.fields = Some(fields);
            form.subscriptions = subs;
        }
    }

    fn touch_on_blur(
        &mut self,
        input: &Entity<InputState>,
        field: ServerField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(input, window, move |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Blur) {
                this.touch_server_field(field, cx);
            }
        })
    }

    /// The user left `field`: its error, if any, shows from now on.
    pub fn touch_server_field(&mut self, field: ServerField, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            if form.touched.insert(field) {
                cx.notify();
            }
        }
    }

    /// The Server the form describes, as typed now.
    pub fn server_from_form(&self, cx: &gpui::App) -> Option<(Server, Option<String>)> {
        let form = self.server_dialog.as_ref()?.form.as_ref()?;
        let fields = form.fields.as_ref()?;
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let key = text(&fields.key_path);
        let key = match key.strip_prefix("~/") {
            Some(rest) => crate::workspace::session::home_dir().map(|h| h.join(rest).display().to_string()).unwrap_or(key),
            None => key,
        };
        let mut server = form.initial.clone();
        server.id = form.id.clone().unwrap_or_default();
        server.name = text(&fields.name);
        server.host = text(&fields.host);
        server.port = parse_port(&text(&fields.port)).unwrap_or(22);
        server.user = text(&fields.user);
        server.keepalive_interval_secs = text(&fields.keepalive_secs).parse().unwrap_or(0);
        server.keepalive_max_missed = text(&fields.keepalive_missed).parse().unwrap_or(0);
        server.auth = if form.use_key { AuthMethod::Key { path: key } } else { AuthMethod::Password };
        server.jump_server_id = form.jump.clone();
        server.remember_secret = form.remember;
        let secret = fields.secret.read(cx).value().to_string();
        Some((server, (!secret.is_empty()).then_some(secret)))
    }

    /// The Jump Server typed in the form, if there is one.
    pub fn new_jump_from_form(&self, cx: &gpui::App) -> Option<(Server, Option<String>)> {
        let form = self.server_dialog.as_ref()?.form.as_ref()?;
        let jump = form.new_jump.as_ref()?;
        let fields = jump.fields.as_ref()?;
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let key = text(&fields.key_path);
        let key = match key.strip_prefix("~/") {
            Some(rest) => crate::workspace::session::home_dir().map(|h| h.join(rest).display().to_string()).unwrap_or(key),
            None => key,
        };
        let mut server = Server::new(&text(&fields.name), &text(&fields.host), &text(&fields.user), AuthMethod::Password);
        server.port = parse_port(&text(&fields.port)).unwrap_or(22);
        server.auth = if jump.use_key { AuthMethod::Key { path: key } } else { AuthMethod::Password };
        server.remember_secret = form.remember;
        let secret = fields.secret.read(cx).value().to_string();
        Some((server, (!secret.is_empty()).then_some(secret)))
    }

    /// Every reason the form cannot be saved as typed, by field, from the
    /// values typed now. Errors about a typed Jump Server are on its field.
    pub fn server_form_errors(&self, cx: &gpui::App) -> Vec<(ServerField, String)> {
        let Some((server, _)) = self.server_from_form(cx) else { return Vec::new() };
        let fields = self.server_dialog.as_ref().and_then(|d| d.form.as_ref()).and_then(|f| f.fields.as_ref());
        let port_text = fields.map(|f| f.port.read(cx).value().to_string()).unwrap_or_default();
        let mut errors: Vec<ServerError> = Vec::new();
        let mut jump_errors: Vec<String> = Vec::new();
        if let Some((jump, _)) = self.new_jump_from_form(cx) {
            jump_errors.extend(self.remote.servers.validate_all(&jump).iter().map(|e| format!("Jump Server: {}", e.message())));
            if jump.name == server.name && !jump.name.is_empty() {
                jump_errors.push("The Jump Server needs a different name".into());
            }
            // The Server itself, once the Jump Server exists.
            let mut target = server;
            target.jump_server_id = None;
            errors.extend(self.remote.servers.validate_all(&target));
        } else {
            errors.extend(self.remote.servers.validate_all(&server));
        }
        if let Err(e) = parse_port(&port_text) {
            let at = errors.iter().position(|e| e.field() == Some(ServerField::User)).unwrap_or(errors.len());
            errors.insert(at, e);
        }
        let mut out: Vec<(ServerField, String)> =
            errors.iter().filter_map(|e| Some((e.field()?, e.message()))).collect();
        out.extend(jump_errors.into_iter().map(|m| (ServerField::JumpServer, m)));
        out
    }

    /// The errors to show now: of the fields the user has left, or all of
    /// them once Save was pressed.
    pub fn visible_server_form_errors(&self, cx: &gpui::App) -> Vec<(ServerField, String)> {
        let Some(form) = self.server_dialog.as_ref().and_then(|d| d.form.as_ref()) else { return Vec::new() };
        self.server_form_errors(cx)
            .into_iter()
            .filter(|(field, _)| form.save_attempted || form.touched.contains(field))
            .collect()
    }

    pub fn save_server_form(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            form.save_attempted = true;
        }
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.error = None;
        }
        if !self.server_form_errors(cx).is_empty() {
            cx.notify();
            return;
        }
        let Some((mut server, secret)) = self.server_from_form(cx) else { return };
        let mut created_jump: Option<String> = None;
        if let Some((jump, jump_secret)) = self.new_jump_from_form(cx) {
            match self.save_server(jump, jump_secret, cx) {
                Ok(id) => {
                    server.jump_server_id = Some(id.clone());
                    created_jump = Some(id);
                }
                Err(e) => {
                    if let Some(dialog) = self.server_dialog.as_mut() {
                        dialog.error = Some(format!("Jump Server: {}", e.message()));
                    }
                    cx.notify();
                    return;
                }
            }
        }
        match self.save_server(server, secret, cx) {
            Ok(_) => self.cancel_server_form(cx),
            Err(e) => {
                // The Server was not saved: do not keep its Jump Server either.
                if let Some(id) = created_jump {
                    let _ = self.delete_server(&id, cx);
                }
                if let Some(dialog) = self.server_dialog.as_mut() {
                    dialog.error = Some(e.message());
                }
                cx.notify();
            }
        }
    }

    pub fn delete_server_from_dialog(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let error = self.delete_server(server_id, cx).err().map(|e| e.message());
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.error = error;
        }
        cx.notify();
    }

    /// Stop trusting the host key of a Server: the next connection asks again.
    pub fn forget_server_host_key(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let Some(server) = self.remote.servers.get(server_id).cloned() else { return };
        {
            let mut hosts = self.remote.known_hosts.lock().unwrap();
            hosts.forget(&server.address());
            let _ = hosts.save_to(&super::remote::known_hosts_path());
        }
        self.reconnect_server_sources(server_id);
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.error = Some(format!("The host key of {} is no longer trusted; it will be asked again.", server.name));
        }
        cx.notify();
    }
}
