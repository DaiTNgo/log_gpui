//! Logic: the Servers dialog: the list, and the form that adds or edits one.

use super::AppView;
use crate::remote::{AgentSocket, AuthMethod, Server, ServerError, ServerField, TestStep, parse_port};
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

/// How the Server is signed in to, as chosen in the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    Key,
    Password,
    Agent,
    Interactive,
}

/// Which ssh-agent socket the form's Agent sign-in uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentChoice {
    System,
    OnePassword,
    Secretive,
    Path,
}

/// The fields of the form that the Jump Server form does not have.
pub struct ExtraFields {
    pub group: Entity<InputState>,
    /// Comma separated.
    pub tags: Entity<InputState>,
    pub agent_path: Entity<InputState>,
    pub preferred_key: Entity<InputState>,
}

/// What a Test connection sends back to the form.
pub enum TestEvent {
    Step(TestStep),
    /// The test ended; the message when it failed.
    Done(Option<String>),
}

/// A Test connection, running or done.
pub struct TestRun {
    pub steps: Vec<TestStep>,
    pub running: bool,
    pub error: Option<String>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl TestRun {
    fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.running = false;
    }
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
    pub auth: AuthKind,
    pub agent: AgentChoice,
    pub extra: Option<ExtraFields>,
    pub test: Option<TestRun>,
    /// What the Jump Server chooser is searched with.
    pub jump_search: Option<Entity<InputState>>,
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
    /// The search field of the list.
    pub search: Option<Entity<InputState>>,
    /// Importing from `~/.ssh/config`.
    pub import: Option<super::ssh_import::ImportDialog>,
    pub error: Option<String>,
    /// A message that is not an error (something done), shown muted.
    pub info: Option<String>,
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
        dialog.info = None;
        let (auth, agent) = match &initial.auth {
            AuthMethod::Key { .. } => (AuthKind::Key, AgentChoice::System),
            AuthMethod::Password => (AuthKind::Password, AgentChoice::System),
            AuthMethod::KeyboardInteractive => (AuthKind::Interactive, AgentChoice::System),
            AuthMethod::Agent { socket, .. } => (
                AuthKind::Agent,
                match socket {
                    AgentSocket::System => AgentChoice::System,
                    AgentSocket::OnePassword => AgentChoice::OnePassword,
                    AgentSocket::Secretive => AgentChoice::Secretive,
                    AgentSocket::Path(_) => AgentChoice::Path,
                },
            ),
        };
        dialog.form = Some(ServerForm {
            id,
            auth,
            agent,
            extra: None,
            test: None,
            jump_search: None,
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
            if let Some(test) = dialog.form.as_mut().and_then(|f| f.test.as_mut()) {
                test.stop();
            }
            dialog.form = None;
            dialog.error = None;
        }
        cx.notify();
    }

    pub fn set_server_auth(&mut self, auth: AuthKind, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            form.auth = auth;
            form.test = None;
        }
        cx.notify();
    }

    /// Put a suggested Server Group in the field.
    pub fn set_server_group(&mut self, group: &str, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.server_dialog.as_ref().and_then(|d| d.form.as_ref()).and_then(|f| f.extra.as_ref()).map(|e| e.group.clone());
        if let Some(input) = input {
            input.update(cx, |state, cx| state.set_value(group.to_string(), window, cx));
        }
        cx.notify();
    }

    pub fn set_agent_choice(&mut self, choice: AgentChoice, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            form.agent = choice;
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
        self.sync_ssh_import(window, cx);
        if self.server_dialog.as_ref().is_some_and(|d| d.search.is_none()) {
            let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search Servers"));
            cx.subscribe_in(&search, window, |this: &mut Self, input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => {
                    let query = input.read(cx).value().to_string();
                    let first = crate::remote::server_search::first_match(&this.remote.servers.servers, &query)
                        .map(|s| s.id.clone());
                    if let Some(id) = first {
                        this.edit_server_form(&id, cx);
                    }
                }
                _ => {}
            })
            .detach();
            if let Some(dialog) = self.server_dialog.as_mut() {
                dialog.search = Some(search);
            }
        }
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
            AuthMethod::Password | AuthMethod::Agent { .. } | AuthMethod::KeyboardInteractive => {
                "~/.ssh/id_ed25519".to_string()
            }
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
        let (agent_path, preferred_key) = match &initial.auth {
            AuthMethod::Agent { socket, preferred_key } => (
                match socket {
                    AgentSocket::Path(p) => p.clone(),
                    _ => String::new(),
                },
                preferred_key.clone().unwrap_or_default(),
            ),
            _ => (String::new(), String::new()),
        };
        let extra = ExtraFields {
            group: field("prod", initial.group.clone().unwrap_or_default(), window, cx),
            tags: field("eu, web", initial.tags.join(", "), window, cx),
            agent_path: field("/path/to/agent.sock", agent_path, window, cx),
            preferred_key: field("~/.ssh/id_ed25519.pub or SHA256:\u{2026}", preferred_key, window, cx),
        };
        let jump_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search Servers"));
        cx.subscribe_in(&jump_search, window, |_this: &mut Self, _input, event: &InputEvent, _window, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
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
            form.extra = Some(extra);
            form.jump_search = Some(jump_search);
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
        server.auth = match form.auth {
            AuthKind::Key => AuthMethod::Key { path: key },
            AuthKind::Password => AuthMethod::Password,
            AuthKind::Interactive => AuthMethod::KeyboardInteractive,
            AuthKind::Agent => {
                let extra = form.extra.as_ref();
                let typed = |input: Option<&Entity<InputState>>| {
                    input.map(|i| i.read(cx).value().trim().to_string()).unwrap_or_default()
                };
                let socket = match form.agent {
                    AgentChoice::System => AgentSocket::System,
                    AgentChoice::OnePassword => AgentSocket::OnePassword,
                    AgentChoice::Secretive => AgentSocket::Secretive,
                    AgentChoice::Path => AgentSocket::Path(typed(extra.map(|e| &e.agent_path))),
                };
                let preferred = typed(extra.map(|e| &e.preferred_key));
                AuthMethod::Agent { socket, preferred_key: (!preferred.is_empty()).then_some(preferred) }
            }
        };
        if let Some(extra) = form.extra.as_ref() {
            let group = text(&extra.group);
            server.group = (!group.is_empty()).then_some(group);
            let mut tags: Vec<String> = Vec::new();
            for tag in text(&extra.tags).split(',').map(str::trim).filter(|t| !t.is_empty()) {
                if !tags.iter().any(|t| t == tag) {
                    tags.push(tag.to_string());
                }
            }
            server.tags = tags;
        }
        server.jump_server_id = form.jump.clone();
        server.remember_secret = form.remember;
        // A secret only counts for sign-ins that have one.
        let secret = if server.has_secret() { fields.secret.read(cx).value().to_string() } else { String::new() };
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

    /// Test connection: go through the whole chain with the values typed
    /// now (nothing is saved) and report each step of each Server.
    pub fn test_server_connection(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()) {
            // Errors show; a test needs values that could be saved.
            form.save_attempted = true;
        }
        if !self.server_form_errors(cx).is_empty() {
            cx.notify();
            return;
        }
        let Some((mut server, secret)) = self.server_from_form(cx) else { return };
        let mut chain: Vec<Server> = Vec::new();
        let mut secrets: std::collections::HashMap<String, String> = Default::default();
        if let Some((mut jump, jump_secret)) = self.new_jump_from_form(cx) {
            jump.id = "test-jump".into();
            if let Some(secret) = jump_secret {
                secrets.insert(jump.id.clone(), secret);
            }
            chain.push(jump);
        } else if let Some(jump_id) = server.jump_server_id.clone() {
            match self.remote.servers.chain(&jump_id) {
                Ok(jumps) => chain.extend(jumps.into_iter().cloned()),
                Err(e) => {
                    if let Some(dialog) = self.server_dialog.as_mut() {
                        dialog.error = Some(e.message());
                    }
                    cx.notify();
                    return;
                }
            }
        }
        if server.id.is_empty() {
            server.id = "test-server".into();
        }
        if let Some(secret) = secret {
            secrets.insert(server.id.clone(), secret);
        }
        chain.push(server);

        let hub = self.remote_hub(cx);
        let (tx, rx) = async_channel::unbounded::<TestEvent>();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                let steps = tx.clone();
                let outcome = hub
                    .test_chain(chain, secrets, &move |step| {
                        let _ = steps.try_send(TestEvent::Step(step));
                    })
                    .await;
                let _ = tx.send(TestEvent::Done(outcome.err().map(|e| e.message()))).await;
            }
        });
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.error = None;
            if let Some(form) = dialog.form.as_mut() {
                if let Some(old) = form.test.as_mut() {
                    old.stop();
                }
                form.test = Some(TestRun { steps: Vec::new(), running: true, error: None, task: Some(task) });
            }
        }
        cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                let done = matches!(event, TestEvent::Done(_));
                let alive = this
                    .update(cx, |this, cx| {
                        let test = this.server_dialog.as_mut().and_then(|d| d.form.as_mut()).and_then(|f| f.test.as_mut());
                        if let Some(test) = test {
                            match event {
                                TestEvent::Step(step) => test.steps.push(step),
                                TestEvent::Done(error) => {
                                    test.running = false;
                                    test.error = error;
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_ok();
                if done || !alive {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// Stop a running test; its connection is closed.
    pub fn cancel_server_test(&mut self, cx: &mut Context<Self>) {
        if let Some(test) = self.server_dialog.as_mut().and_then(|d| d.form.as_mut()).and_then(|f| f.test.as_mut()) {
            test.stop();
        }
        cx.notify();
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

    /// Ask before deleting a Server. One that is in use is refused at once,
    /// saying what uses it.
    pub fn request_delete_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let mut users = self.server_users(server_id);
        for s in &self.remote.servers.servers {
            if s.jump_server_id.as_deref() == Some(server_id) {
                users.push(format!("Jump Server of {}", s.name));
            }
        }
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.info = None;
            dialog.error = (!users.is_empty()).then(|| ServerError::InUse(users.clone()).message());
        }
        if users.is_empty() {
            self.confirm = Some(crate::app::ConfirmAction::DeleteServer { server_id: server_id.to_string() });
        }
        cx.notify();
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
            dialog.error = None;
            dialog.info = Some(format!("The host key of {} is no longer trusted; it will be asked again.", server.name));
        }
        cx.notify();
    }
}

/// One line per Server of the chain for the Test connection result: what
/// worked, in order, and the step that failed (if any) marked. `true` when
/// that Server's line has no failure.
pub fn test_lines(steps: &[TestStep]) -> Vec<(String, bool)> {
    use crate::remote::TestStepKind;
    let mut hops: Vec<(usize, &str)> = Vec::new();
    for step in steps {
        if !hops.iter().any(|(hop, _)| *hop == step.hop) {
            hops.push((step.hop, step.server_name.as_str()));
        }
    }
    hops.into_iter()
        .map(|(hop, name)| {
            let mut parts: Vec<String> = Vec::new();
            let mut ok = true;
            for step in steps.iter().filter(|s| s.hop == hop) {
                match &step.result {
                    Ok(detail) => parts.push(detail.clone()),
                    Err(message) => {
                        ok = false;
                        parts.push(match step.kind {
                            TestStepKind::Sftp => format!("\u{2717} {message}"),
                            TestStepKind::Tcp => format!("\u{2717} connection: {message}"),
                            TestStepKind::HostKey => format!("\u{2717} host key: {message}"),
                            TestStepKind::Auth => format!("\u{2717} sign-in: {message}"),
                        });
                    }
                }
            }
            (format!("{name}: {}", parts.join(", ")), ok)
        })
        .collect()
}
