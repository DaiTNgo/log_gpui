//! Logic: importing Servers from `~/.ssh/config` and refreshing them from
//! it. The file is read when the dialog opens and never otherwise.

use super::AppView;
use crate::remote::ssh_config::{
    Candidate, ConfigFs, Env, FieldChange, RealFs, RefreshRow, Skipped, apply_changes, apply_import, candidates,
    read_config, refresh,
};
use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;
use std::collections::{HashMap, HashSet};

pub struct ImportDialog {
    /// `~/.ssh/config` exists.
    pub config_found: bool,
    /// Hosts not imported yet.
    pub new: Vec<Candidate>,
    /// Ticked rows of `new`, by alias.
    pub ticked: HashSet<String>,
    /// Name fields of the rows whose name is taken, by alias (created
    /// while rendering).
    pub name_inputs: HashMap<String, Entity<InputState>>,
    pub group_input: Option<Entity<InputState>>,
    pub skipped: Vec<Skipped>,
    /// Servers imported earlier, compared with the config now.
    pub known: Vec<RefreshRow>,
    /// Ticked changes: Server id and field.
    pub ticked_changes: HashSet<(String, &'static str)>,
    /// Every host of the config, for resolving Jump Servers.
    all: Vec<Candidate>,
}

impl ImportDialog {
    fn empty() -> Self {
        Self {
            config_found: false,
            new: Vec::new(),
            ticked: HashSet::new(),
            name_inputs: HashMap::new(),
            group_input: None,
            skipped: Vec::new(),
            known: Vec::new(),
            ticked_changes: HashSet::new(),
            all: Vec::new(),
        }
    }

    /// Read the config with `fs` and compare it with the Servers.
    pub fn read(fs: &dyn ConfigFs, env: &Env, store: &crate::remote::ServerStore) -> Self {
        let Some(parsed) = read_config(fs, env) else { return Self::empty() };
        let all = candidates(&parsed, fs, env, store);
        let (known, new) = refresh(store, &all);
        let ticked = new.iter().filter(|c| c.checked).map(|c| c.alias.clone()).collect();
        let ticked_changes = known
            .iter()
            .filter_map(|row| match row {
                RefreshRow::Changed { server_id, changes, .. } => {
                    Some(changes.iter().map(|c| (server_id.clone(), c.field)).collect::<Vec<_>>())
                }
                _ => None,
            })
            .flatten()
            .collect();
        Self {
            config_found: true,
            new,
            ticked,
            name_inputs: HashMap::new(),
            group_input: None,
            skipped: parsed.skipped,
            known,
            ticked_changes,
            all,
        }
    }

    /// Whether Import has anything to do.
    pub fn has_work(&self) -> bool {
        !self.ticked.is_empty() || !self.ticked_changes.is_empty()
    }
}

impl AppView {
    /// "Import from ~/.ssh/config…" and "Refresh from ~/.ssh/config".
    pub fn open_ssh_import(&mut self, cx: &mut Context<Self>) {
        self.open_ssh_import_with(&RealFs, &Env::current(), cx);
    }

    pub fn open_ssh_import_with(&mut self, fs: &dyn ConfigFs, env: &Env, cx: &mut Context<Self>) {
        let dialog = ImportDialog::read(fs, env, &self.remote.servers);
        if let Some(server_dialog) = self.server_dialog.as_mut() {
            server_dialog.import = Some(dialog);
            server_dialog.error = None;
            server_dialog.info = None;
        }
        cx.notify();
    }

    pub fn close_ssh_import(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.import = None;
        }
        cx.notify();
    }

    /// Create the fields of a freshly opened import dialog.
    pub fn sync_ssh_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(import) = self.server_dialog.as_ref().and_then(|d| d.import.as_ref()) else { return };
        let missing: Vec<(String, String)> = import
            .new
            .iter()
            .filter(|c| c.name_taken && !import.name_inputs.contains_key(&c.alias))
            .map(|c| (c.alias.clone(), c.name.clone()))
            .collect();
        let need_group = import.group_input.is_none() && import.config_found;
        if missing.is_empty() && !need_group {
            return;
        }
        let mut made = Vec::new();
        for (alias, name) in missing {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Server name"));
            input.update(cx, |state, cx| state.set_value(name, window, cx));
            made.push((alias, input));
        }
        let group = need_group.then(|| cx.new(|cx| InputState::new(window, cx).placeholder("Server Group (optional)")));
        if let Some(import) = self.server_dialog.as_mut().and_then(|d| d.import.as_mut()) {
            import.name_inputs.extend(made);
            if group.is_some() {
                import.group_input = group;
            }
        }
    }

    pub fn toggle_import_row(&mut self, alias: &str, cx: &mut Context<Self>) {
        if let Some(import) = self.server_dialog.as_mut().and_then(|d| d.import.as_mut())
            && !import.ticked.remove(alias)
        {
            import.ticked.insert(alias.to_string());
        }
        cx.notify();
    }

    pub fn toggle_import_change(&mut self, server_id: &str, field: &'static str, cx: &mut Context<Self>) {
        if let Some(import) = self.server_dialog.as_mut().and_then(|d| d.import.as_mut()) {
            let key = (server_id.to_string(), field);
            if !import.ticked_changes.remove(&key) {
                import.ticked_changes.insert(key);
            }
        }
        cx.notify();
    }

    /// Import the ticked hosts and apply the ticked changes.
    pub fn apply_ssh_import(&mut self, cx: &mut Context<Self>) {
        let Some(import) = self.server_dialog.as_ref().and_then(|d| d.import.as_ref()) else { return };
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let group = import.group_input.as_ref().map(text).filter(|g| !g.is_empty());
        // The names as edited on the rows.
        let mut all = import.all.clone();
        for candidate in &mut all {
            if let Some(input) = import.name_inputs.get(&candidate.alias) {
                let name = text(input);
                if !name.is_empty() {
                    candidate.name = name;
                }
            }
        }
        let selected: Vec<String> = import.new.iter().filter(|c| import.ticked.contains(&c.alias)).map(|c| c.alias.clone()).collect();
        let changes: Vec<(String, &'static str)> = import.ticked_changes.iter().cloned().collect();

        let mut store = self.remote.servers.clone();
        let report = apply_import(&mut store, &all, &selected, group.as_deref());
        if !report.created.is_empty() {
            self.remote.servers = store;
            let _ = self.remote.servers.save_to(&super::remote::servers_path());
            *self.remote.shared_servers.lock().unwrap() = self.remote.servers.clone();
        }
        // Changes go through the same path as editing a Server, which
        // reconnects the files of one in use.
        let mut by_server: HashMap<String, Vec<&'static str>> = HashMap::new();
        for (id, field) in changes {
            by_server.entry(id).or_default().push(field);
        }
        let mut applied = 0;
        for (id, fields) in by_server {
            let Some(mut server) = self.remote.servers.get(&id).cloned() else { continue };
            let Some(crate::remote::Origin::SshConfig { alias }) = server.origin.clone() else { continue };
            let Some(candidate) = all.iter().find(|c| c.alias == alias) else { continue };
            apply_changes(&mut server, candidate, &fields, &self.remote.servers);
            if self.save_server(server, None, cx).is_ok() {
                applied += 1;
            }
        }
        let message = match (report.created.len(), applied) {
            (0, 0) => None,
            (n, 0) => Some(format!("Imported {n} Server{}", if n == 1 { "" } else { "s" })),
            (0, m) => Some(format!("Updated {m} Server{}", if m == 1 { "" } else { "s" })),
            (n, m) => Some(format!("Imported {n}, updated {m}")),
        };
        if let Some(dialog) = self.server_dialog.as_mut() {
            dialog.import = None;
            dialog.info = message;
        }
        cx.notify();
    }
}

/// The text of a change for its checkbox.
pub fn change_text(change: &FieldChange) -> String {
    change.describe()
}
