//! Logic: Quick Connect in the file picker. One typed string names a Server
//! and a path; a Server that does not exist yet is made in memory and only
//! saved when Done adds a file of its own.

use super::AppView;
use super::remote_picker::QuickLogin;
use crate::remote::quick::{self, Resolution};
use crate::remote::{AuthMethod, Server, ServerStore};
use gpui::{AppContext as _, Context, Window};
use gpui_component::input::{InputEvent, InputState};

impl AppView {
    /// The Servers the picker can use: the saved ones and the ones made by
    /// Quick Connect that are not saved yet.
    pub(crate) fn picker_store(&self) -> ServerStore {
        let mut store = self.remote.servers.clone();
        if let Some(picker) = self.remote.picker.as_ref() {
            store.servers.extend(picker.quick_servers.iter().cloned());
        }
        store
    }

    /// A Server by id, saved or made by Quick Connect.
    pub(crate) fn picker_server(&self, id: &str) -> Option<Server> {
        self.remote
            .servers
            .get(id)
            .or_else(|| self.remote.picker.as_ref().and_then(|p| p.quick_servers.iter().find(|s| s.id == id)))
            .cloned()
    }

    /// Enter in the Quick Connect field.
    pub fn picker_quick_connect(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.remote.picker.as_ref().and_then(|p| p.quick_input.clone()) else { return };
        let text = input.read(cx).value().to_string();
        self.picker_quick_connect_text(&text, cx);
    }

    pub fn picker_quick_connect_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        let target = match quick::parse(text) {
            Ok(target) => target,
            Err(message) => {
                if let Some(picker) = self.remote.picker.as_mut() {
                    picker.error = Some(message);
                    picker.info = None;
                }
                cx.notify();
                return;
            }
        };
        let local_user = std::env::var("USER").unwrap_or_default();
        let store = self.picker_store();
        let server_id = match quick::resolve(&store, &target, &local_user) {
            Resolution::Existing(id) => id,
            Resolution::New(mut server) => {
                let Some(picker) = self.remote.picker.as_mut() else { return };
                // The same string again reuses the Server made before.
                match picker.quick_servers.iter().find(|s| s.host == server.host && s.port == server.port && s.user == server.user) {
                    Some(existing) => existing.id.clone(),
                    None => {
                        server.id = format!("{}{}", quick::TEMP_ID_PREFIX, picker.quick_servers.len() + 1000);
                        let id = server.id.clone();
                        picker.quick_servers.push(server);
                        id
                    }
                }
            }
        };
        if let Some(picker) = self.remote.picker.as_mut() {
            picker.quick_pending = Some(target.path.clone());
            picker.quick_login = None;
            picker.error = None;
            picker.info = None;
        }
        self.picker_select_server(&server_id, cx);
    }

    /// The agent did not get into this Server: ask for a password or a key.
    pub(crate) fn open_quick_login(&mut self, server_id: &str) {
        let name = self.picker_server(server_id).map(|s| s.name).unwrap_or_default();
        if let Some(picker) = self.remote.picker.as_mut() {
            picker.error = None;
            picker.quick_login = Some(QuickLogin {
                server_id: server_id.to_string(),
                server_name: name,
                use_key: false,
                remember: false,
                secret: None,
                key_path: None,
                error: None,
            });
        }
    }

    pub fn quick_login_set_key(&mut self, use_key: bool, cx: &mut Context<Self>) {
        if let Some(login) = self.remote.picker.as_mut().and_then(|p| p.quick_login.as_mut()) {
            login.use_key = use_key;
            login.error = None;
        }
        cx.notify();
    }

    pub fn quick_login_toggle_remember(&mut self, cx: &mut Context<Self>) {
        if let Some(login) = self.remote.picker.as_mut().and_then(|p| p.quick_login.as_mut()) {
            login.remember = !login.remember;
        }
        cx.notify();
    }

    pub fn quick_login_cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(picker) = self.remote.picker.as_mut() {
            picker.quick_login = None;
            picker.quick_pending = None;
        }
        cx.notify();
    }

    /// Sign in with what was typed (always after the agent was refused), and
    /// connect again.
    pub fn quick_login_submit(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_ref() else { return };
        let Some(login) = picker.quick_login.as_ref() else { return };
        let read = |input: &Option<gpui::Entity<InputState>>| {
            input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default()
        };
        let secret = read(&login.secret);
        let key = read(&login.key_path).trim().to_string();
        let (use_key, remember, server_id) = (login.use_key, login.remember, login.server_id.clone());
        let problem = if use_key && key.is_empty() {
            Some("Enter the path of the key file")
        } else if !use_key && secret.is_empty() {
            Some("Enter the password")
        } else {
            None
        };
        if let Some(problem) = problem {
            if let Some(login) = self.remote.picker.as_mut().and_then(|p| p.quick_login.as_mut()) {
                login.error = Some(problem.into());
            }
            cx.notify();
            return;
        }
        let key = match key.strip_prefix("~/") {
            Some(rest) => crate::workspace::session::home_dir().map(|h| h.join(rest).display().to_string()).unwrap_or(key),
            None => key,
        };
        let hub = self.remote_hub(cx);
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(server) = picker.quick_servers.iter_mut().find(|s| s.id == server_id) else { return };
        server.auth = if use_key { AuthMethod::Key { path: key } } else { AuthMethod::Password };
        server.remember_secret = false;
        if !secret.is_empty() {
            // Held for this run; the Keychain only gets it when the Server
            // is saved (its id is not known before).
            hub.provide_secret(server, &secret);
            picker.quick_secrets.insert(server_id.clone(), (secret, remember));
        }
        server.remember_secret = remember;
        picker.quick_login = None;
        self.picker_select_server(&server_id, cx);
    }

    /// Create the fields of the Quick Connect field and of the login form.
    pub fn sync_quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if picker.quick_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("user@host[:port]:/path  or  server:/path"));
            cx.subscribe_in(&input, window, |this: &mut Self, _input, event: &InputEvent, _window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.picker_quick_connect(cx);
                }
            })
            .detach();
            picker.quick_input = Some(input);
        }
        let Some(login) = picker.quick_login.as_mut() else { return };
        if login.secret.is_none() {
            let secret = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Password or key passphrase"));
            cx.subscribe_in(&secret, window, |this: &mut Self, _input, event: &InputEvent, _window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.quick_login_submit(cx);
                }
            })
            .detach();
            secret.update(cx, |state, cx| state.focus(window, cx));
            login.secret = Some(secret);
        }
        if login.key_path.is_none() {
            let key = cx.new(|cx| InputState::new(window, cx).placeholder("~/.ssh/id_ed25519"));
            login.key_path = Some(key);
        }
    }

    /// Done: save the Servers made by Quick Connect that have a file in the
    /// waiting list and point those files at the saved Server. The others are
    /// dropped.
    pub(crate) fn save_quick_servers(&mut self, picker: &mut super::remote_picker::RemotePicker, cx: &mut Context<Self>) {
        let temp_ids: Vec<String> = picker
            .staged
            .iter()
            .map(|s| s.server_id().to_string())
            .filter(|id| quick::is_temporary(id))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        for temp_id in temp_ids {
            let Some(mut server) = picker.quick_servers.iter().find(|s| s.id == temp_id).cloned() else { continue };
            server.id = String::new();
            server.name = free_server_name(&self.remote.servers, &server.name);
            let secret = picker.quick_secrets.get(&temp_id).cloned();
            match self.save_server(server, None, cx) {
                Ok(real_id) => {
                    if let (Some((secret, _)), Some(saved)) = (secret, self.remote.servers.get(&real_id).cloned()) {
                        // Into the Keychain when "Remember" was ticked.
                        let hub = self.remote_hub(cx);
                        hub.provide_secret(&saved, &secret);
                    }
                    for item in &mut picker.staged {
                        if item.server_id() == temp_id {
                            item.set_server_id(&real_id);
                        }
                    }
                }
                Err(_) => picker.staged.retain(|s| s.server_id() != temp_id),
            }
        }
    }
}

/// `wanted`, or `wanted (2)`… when another Server has the name.
fn free_server_name(store: &ServerStore, wanted: &str) -> String {
    if store.by_name(wanted).is_none() {
        return wanted.to_string();
    }
    (2..).map(|n| format!("{wanted} ({n})")).find(|n| store.by_name(n).is_none()).expect("a free name exists")
}
