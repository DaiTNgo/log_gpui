//! Reading `~/.ssh/config` once, when the user imports: the `Host` entries
//! become Servers and Jump Servers. The file is never read while connecting
//! and never written. Only the options that decide where and how to sign in
//! are understood; the rest is listed as skipped.

use super::browse::glob_match;
use super::server::{AgentSocket, AuthMethod, MAX_JUMPS, Origin, Server, ServerStore};
use std::collections::{HashMap, HashSet};

/// How deep `Include` may nest before it is cut off.
const MAX_INCLUDE_DEPTH: usize = 16;

/// What the parser reads of the machine, so tests need no real files.
pub trait ConfigFs {
    fn read(&self, path: &str) -> Option<String>;
    /// Names (not paths) of the files in a folder.
    fn list(&self, dir: &str) -> Vec<String>;
    fn exists(&self, path: &str) -> bool;
}

pub struct RealFs;

impl ConfigFs for RealFs {
    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}

/// The user and home folder `%u`, `%d` and `~` stand for.
#[derive(Debug, Clone)]
pub struct Env {
    pub home: String,
    pub local_user: String,
}

impl Env {
    pub fn current() -> Self {
        Self {
            home: std::env::var("HOME").unwrap_or_default(),
            local_user: std::env::var("USER").unwrap_or_default(),
        }
    }

    fn config_path(&self) -> String {
        format!("{}/.ssh/config", self.home)
    }

    fn ssh_dir(&self) -> String {
        format!("{}/.ssh", self.home)
    }
}

/// What the config says about one `Host` alias, after applying every block
/// that matches it ("first value wins").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigHost {
    pub alias: String,
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_files: Vec<String>,
    pub identity_agent: Option<String>,
    pub proxy_jump: Option<String>,
    /// Options that matter for connecting and are not supported.
    pub unsupported: Vec<String>,
}

/// Something in the file that is left out, for the "Skipped" list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub what: String,
    pub why: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedConfig {
    /// Hosts with a concrete name, in file order.
    pub hosts: Vec<ConfigHost>,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Host(Vec<String>),
    /// A `Match` line, kept as written.
    Match(String),
    Option { key: String, values: Vec<String> },
}

/// Split one line into its keyword and its arguments: `Key value`,
/// `Key=value`, `Key = value`, double quotes around an argument with
/// spaces, `#` starting a comment where an argument could start.
pub fn tokenize_line(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let key_end = line.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(line.len());
    let key = line[..key_end].to_lowercase();
    let mut rest = line[key_end..].trim_start();
    if let Some(after) = rest.strip_prefix('=') {
        rest = after.trim_start();
    }
    let mut values = Vec::new();
    let mut chars = rest.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        match chars.peek() {
            None => break,
            Some('#') => break,
            _ => {}
        }
        let mut value = String::new();
        let mut quoted = false;
        while let Some(&c) = chars.peek() {
            if c == '"' {
                quoted = !quoted;
                chars.next();
            } else if c.is_whitespace() && !quoted {
                break;
            } else {
                value.push(c);
                chars.next();
            }
        }
        values.push(value);
    }
    Some((key, values))
}

fn read_items(fs: &dyn ConfigFs, env: &Env, text: &str, depth: usize, items: &mut Vec<Item>) {
    for line in text.lines() {
        let Some((key, values)) = tokenize_line(line) else { continue };
        match key.as_str() {
            "host" => items.push(Item::Host(values)),
            "match" => items.push(Item::Match(values.join(" "))),
            "include" => {
                if depth >= MAX_INCLUDE_DEPTH {
                    continue;
                }
                for pattern in values {
                    for path in include_paths(fs, env, &pattern) {
                        if let Some(text) = fs.read(&path) {
                            read_items(fs, env, &text, depth + 1, items);
                        }
                    }
                }
            }
            _ => items.push(Item::Option { key, values }),
        }
    }
}

/// The files an `Include` pattern stands for: relative paths start from
/// `~/.ssh`, the file name part may be a glob.
fn include_paths(fs: &dyn ConfigFs, env: &Env, pattern: &str) -> Vec<String> {
    let expanded = tilde(pattern, env);
    let full = if expanded.starts_with('/') { expanded } else { format!("{}/{expanded}", env.ssh_dir()) };
    let (dir, name) = match full.rsplit_once('/') {
        Some((dir, name)) => (dir.to_string(), name.to_string()),
        None => return Vec::new(),
    };
    if !name.contains(['*', '?', '[']) {
        return if fs.exists(&full) { vec![full] } else { Vec::new() };
    }
    fs.list(&dir).into_iter().filter(|n| glob_match(&name, n)).map(|n| format!("{dir}/{n}")).collect()
}

/// Whether a `Host` pattern list matches `alias`: some pattern matches and
/// none of the `!` patterns does.
fn host_matches(patterns: &[String], alias: &str) -> bool {
    let alias = alias.to_lowercase();
    let mut hit = false;
    for pattern in patterns {
        match pattern.strip_prefix('!') {
            Some(negated) => {
                if glob_match(&negated.to_lowercase(), &alias) {
                    return false;
                }
            }
            None => hit |= glob_match(&pattern.to_lowercase(), &alias),
        }
    }
    hit
}

fn is_pattern(name: &str) -> bool {
    name.contains(['*', '?', '!'])
}

/// Options that decide how a connection is made and that this app does not
/// support.
const UNSUPPORTED_OPTIONS: [(&str, &str); 5] = [
    ("proxycommand", "ProxyCommand"),
    ("localforward", "LocalForward"),
    ("remoteforward", "RemoteForward"),
    ("dynamicforward", "DynamicForward"),
    ("certificatefile", "CertificateFile"),
];

/// Read the config at `~/.ssh/config`; `None` when there is no such file.
pub fn read_config(fs: &dyn ConfigFs, env: &Env) -> Option<ParsedConfig> {
    let text = fs.read(&env.config_path())?;
    Some(parse_config(fs, env, &text))
}

pub fn parse_config(fs: &dyn ConfigFs, env: &Env, text: &str) -> ParsedConfig {
    let mut items = Vec::new();
    read_items(fs, env, text, 0, &mut items);

    let mut parsed = ParsedConfig::default();
    let mut aliases: Vec<String> = Vec::new();
    for item in &items {
        match item {
            Item::Host(names) => {
                let concrete: Vec<&String> = names.iter().filter(|n| !is_pattern(n)).collect();
                if concrete.is_empty() {
                    parsed.skipped.push(Skipped {
                        what: format!("Host {}", names.join(" ")),
                        why: "pattern only; its values still apply to the hosts it matches".into(),
                    });
                }
                for name in concrete {
                    if !aliases.contains(name) {
                        aliases.push(name.clone());
                    }
                }
            }
            Item::Match(rest) => parsed.skipped.push(Skipped {
                what: format!("Match {rest}"),
                why: "Match blocks are not supported; nothing is taken from them".into(),
            }),
            Item::Option { .. } => {}
        }
    }
    parsed.hosts = aliases.iter().map(|alias| resolve_host(&items, env, alias)).collect();
    parsed
}

fn resolve_host(items: &[Item], env: &Env, alias: &str) -> ConfigHost {
    let mut host = ConfigHost { alias: alias.to_string(), ..Default::default() };
    // Before the first Host line every option applies to every host.
    let mut active = true;
    let mut seen: HashSet<&str> = HashSet::new();
    for item in items {
        match item {
            Item::Host(patterns) => active = host_matches(patterns, alias),
            Item::Match(_) => active = false,
            Item::Option { key, values } if active => {
                let value = values.join(" ");
                match key.as_str() {
                    "hostname" if seen.insert("hostname") => host.host_name = Some(value),
                    "user" if seen.insert("user") => host.user = Some(value),
                    "port" if seen.insert("port") => host.port = value.trim().parse().ok(),
                    "identityagent" if seen.insert("identityagent") => host.identity_agent = Some(value),
                    "proxyjump" if seen.insert("proxyjump") => host.proxy_jump = Some(value),
                    "identityfile" => {
                        if let Some(first) = values.first() {
                            host.identity_files.push(first.clone());
                        }
                    }
                    other => {
                        if let Some((_, shown)) = UNSUPPORTED_OPTIONS.iter().find(|(k, _)| *k == other)
                            && !host.unsupported.iter().any(|u| u == shown)
                        {
                            host.unsupported.push((*shown).to_string());
                        }
                    }
                }
            }
            Item::Option { .. } => {}
        }
    }
    // Tokens and `~`, once the values are known.
    let remote_host = host.host_name.as_deref().map(|h| expand_tokens(h, alias, alias, &host.user, env));
    host.host_name = remote_host;
    let host_for_tokens = host.host_name.clone().unwrap_or_else(|| alias.to_string());
    let user = host.user.clone();
    host.identity_files = host
        .identity_files
        .iter()
        .map(|f| tilde(&expand_tokens(f, alias, &host_for_tokens, &user, env), env))
        .collect();
    host.identity_agent = host
        .identity_agent
        .as_deref()
        .map(|a| tilde(&expand_tokens(a, alias, &host_for_tokens, &user, env), env));
    host
}

/// `~` and `~/…` as the home folder.
fn tilde(path: &str, env: &Env) -> String {
    if path == "~" {
        env.home.clone()
    } else if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{rest}", env.home)
    } else {
        path.to_string()
    }
}

/// `%h` (the host: `HostName`, or the alias while expanding `HostName`
/// itself), `%u` (local user), `%r` (remote user), `%d` (home), `%%`.
fn expand_tokens(text: &str, alias: &str, host: &str, remote_user: &Option<String>, env: &Env) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('h') => out.push_str(if host.is_empty() { alias } else { host }),
            Some('u') => out.push_str(&env.local_user),
            Some('r') => out.push_str(remote_user.as_deref().unwrap_or(&env.local_user)),
            Some('d') => out.push_str(&env.home),
            Some('%') => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

/// A Server the import would make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The `Host` alias in the config.
    pub alias: String,
    /// The Server's name, which the user may change on the row.
    pub name: String,
    pub server: Server,
    pub warnings: Vec<String>,
    /// Why it cannot be imported (a loop or too many Jump Servers).
    pub error: Option<String>,
    /// Another Server already has this name and it is not this alias.
    pub name_taken: bool,
    /// The Jump Servers it goes through, outermost first.
    pub hops: Vec<Hop>,
    /// Ticked when the dialog opens.
    pub checked: bool,
}

/// One Jump Server in a chain to import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hop {
    /// A Server the app already has.
    Existing { id: String, name: String },
    /// Another `Host` of the config, imported with it.
    Config { alias: String },
    /// A `[user@]host[:port]` with no entry of its own.
    Spec { user: String, host: String, port: u16 },
}

impl Hop {
    pub fn describe(&self) -> String {
        match self {
            Hop::Existing { name, .. } => name.clone(),
            Hop::Config { alias } => alias.clone(),
            Hop::Spec { user, host, port } => spec_name(user, host, *port),
        }
    }
}

/// `user@host`, with `:port` when it is not 22.
pub fn spec_name(user: &str, host: &str, port: u16) -> String {
    if port == 22 { format!("{user}@{host}") } else { format!("{user}@{host}:{port}") }
}

/// Parse `[user@]host[:port]` of a `ProxyJump` entry.
fn parse_jump_spec(text: &str, env: &Env) -> Option<(String, String, u16)> {
    let text = text.trim();
    let text = text.strip_prefix("ssh://").unwrap_or(text);
    let (user, rest) = match text.split_once('@') {
        Some((u, r)) => (u.to_string(), r),
        None => (env.local_user.clone(), text),
    };
    let (host, port) = if let Some(inner) = rest.strip_prefix('[') {
        let (host, after) = inner.split_once(']')?;
        (host.to_string(), after.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(22))
    } else if let Some((h, p)) = rest.rsplit_once(':') {
        (h.to_string(), p.parse().ok()?)
    } else {
        (rest.to_string(), 22)
    };
    (!host.is_empty()).then_some((user, host, port))
}

/// The socket an `IdentityAgent` value stands for.
fn agent_socket_for(value: &str) -> AgentSocket {
    if value == "SSH_AUTH_SOCK" || value == "$SSH_AUTH_SOCK" {
        return AgentSocket::System;
    }
    if value.eq_ignore_ascii_case(super::server::PAGEANT) {
        return AgentSocket::Pageant;
    }
    for preset in [AgentSocket::OnePassword, AgentSocket::Secretive] {
        if preset.resolve().as_deref() == Some(value) {
            return preset;
        }
    }
    AgentSocket::Path(value.to_string())
}

/// How to sign in, from `IdentityAgent` and `IdentityFile`; plus a warning
/// when a key file named is missing.
fn auth_for(host: &ConfigHost, fs: &dyn ConfigFs, warnings: &mut Vec<String>) -> AuthMethod {
    let pub_of = |file: &str| -> Option<String> {
        if file.ends_with(".pub") {
            fs.exists(file).then(|| file.to_string())
        } else {
            let candidate = format!("{file}.pub");
            fs.exists(&candidate).then_some(candidate)
        }
    };
    let agent = host.identity_agent.as_deref().filter(|a| !a.eq_ignore_ascii_case("none"));
    if let Some(agent) = agent {
        let preferred = host.identity_files.iter().find_map(|f| pub_of(f));
        return AuthMethod::Agent { socket: agent_socket_for(agent), preferred_key: preferred };
    }
    let existing: Vec<&String> = host.identity_files.iter().filter(|f| fs.exists(f)).collect();
    if let Some(file) = existing.iter().find(|f| !f.ends_with(".pub")) {
        return AuthMethod::Key { path: (*file).clone() };
    }
    if let Some(public) = existing.first() {
        // Only a public key: the agent holds the private half.
        return AuthMethod::Agent { socket: AgentSocket::System, preferred_key: Some((*public).clone()) };
    }
    if !host.identity_files.is_empty() {
        warnings.push("Key file not found".into());
    }
    AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None }
}

/// Everything the dialog lists: one `Candidate` per concrete `Host`.
pub fn candidates(
    parsed: &ParsedConfig,
    fs: &dyn ConfigFs,
    env: &Env,
    store: &ServerStore,
) -> Vec<Candidate> {
    let by_alias: HashMap<&str, &ConfigHost> = parsed.hosts.iter().map(|h| (h.alias.as_str(), h)).collect();
    parsed.hosts.iter().map(|host| candidate(host, &by_alias, fs, env, store)).collect()
}

fn candidate(
    host: &ConfigHost,
    by_alias: &HashMap<&str, &ConfigHost>,
    fs: &dyn ConfigFs,
    env: &Env,
    store: &ServerStore,
) -> Candidate {
    let mut warnings = Vec::new();
    let auth = auth_for(host, fs, &mut warnings);
    let mut server = Server::new(
        &host.alias,
        host.host_name.as_deref().unwrap_or(&host.alias),
        host.user.as_deref().unwrap_or(&env.local_user),
        auth,
    );
    server.port = host.port.unwrap_or(22);
    server.origin = Some(Origin::SshConfig { alias: host.alias.clone() });
    let proxy_command = host.unsupported.iter().any(|u| u == "ProxyCommand");
    for option in &host.unsupported {
        warnings.push(format!("{option} is not supported"));
    }
    let (hops, error) = match resolve_hops(host, by_alias, env, store) {
        Ok(hops) => (hops, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let name_taken = store
        .by_name(&host.alias)
        .is_some_and(|s| s.origin != Some(Origin::SshConfig { alias: host.alias.clone() }));
    if name_taken {
        warnings.push("Name already used".into());
    }
    let checked = !proxy_command && !name_taken && error.is_none();
    Candidate { alias: host.alias.clone(), name: host.alias.clone(), server, warnings, error, name_taken, hops, checked }
}

/// The chain of a `ProxyJump`, outermost first.
fn resolve_hops(
    host: &ConfigHost,
    by_alias: &HashMap<&str, &ConfigHost>,
    env: &Env,
    store: &ServerStore,
) -> Result<Vec<Hop>, String> {
    let mut visiting = vec![host.alias.clone()];
    let hops = hops_of(host, by_alias, env, store, &mut visiting)?;
    if hops.len() > MAX_JUMPS {
        return Err(format!("More than {MAX_JUMPS} Jump Servers"));
    }
    Ok(hops)
}

fn hops_of(
    host: &ConfigHost,
    by_alias: &HashMap<&str, &ConfigHost>,
    env: &Env,
    store: &ServerStore,
    visiting: &mut Vec<String>,
) -> Result<Vec<Hop>, String> {
    let Some(jump) = host.proxy_jump.as_deref().filter(|j| !j.eq_ignore_ascii_case("none")) else {
        return Ok(Vec::new());
    };
    let mut hops = Vec::new();
    for entry in jump.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        if let Some(existing) = store.by_name(entry) {
            hops.push(Hop::Existing { id: existing.id.clone(), name: existing.name.clone() });
        } else if let Some(other) = by_alias.get(entry) {
            if visiting.iter().any(|v| v == entry) {
                return Err("The Jump Servers loop back on themselves".into());
            }
            visiting.push(entry.to_string());
            hops.extend(hops_of(other, by_alias, env, store, visiting)?);
            visiting.pop();
            hops.push(Hop::Config { alias: entry.to_string() });
        } else {
            let (user, host, port) =
                parse_jump_spec(entry, env).ok_or_else(|| format!("Cannot read the Jump Server \"{entry}\""))?;
            match store.servers.iter().find(|s| s.host == host && s.port == port && s.user == user) {
                Some(existing) => hops.push(Hop::Existing { id: existing.id.clone(), name: existing.name.clone() }),
                None => hops.push(Hop::Spec { user, host, port }),
            }
        }
    }
    Ok(hops)
}

/// A name for a new Server that no other Server has.
fn free_name(store: &ServerStore, wanted: &str) -> String {
    if store.by_name(wanted).is_none() {
        return wanted.to_string();
    }
    (2..).map(|n| format!("{wanted} ({n})")).find(|n| store.by_name(n).is_none()).expect("a free name exists")
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Names of the Servers made, Jump Servers first.
    pub created: Vec<String>,
}

/// Save the ticked candidates (and the Jump Servers they need) into the
/// store. Rows with an `error` or a name another Server already has are left
/// out; no Server of the store is overwritten.
pub fn apply_import(
    store: &mut ServerStore,
    all: &[Candidate],
    selected: &[String],
    group: Option<&str>,
) -> ImportReport {
    let group = group.map(str::trim).filter(|g| !g.is_empty()).map(str::to_string);
    let by_alias: HashMap<&str, &Candidate> = all.iter().map(|c| (c.alias.as_str(), c)).collect();
    let mut report = ImportReport::default();
    // Config alias -> id of the Server made or found for it.
    let mut made: HashMap<String, String> = HashMap::new();
    let mut specs: HashMap<(String, String, u16), String> = HashMap::new();

    for alias in selected {
        let Some(candidate) = by_alias.get(alias.as_str()) else { continue };
        if candidate.error.is_some() || made.contains_key(alias) {
            continue;
        }
        let mut previous: Option<String> = None;
        for hop in &candidate.hops {
            let id = match hop {
                Hop::Existing { id, .. } => id.clone(),
                Hop::Config { alias } => match made.get(alias) {
                    Some(id) => id.clone(),
                    None => {
                        let Some(hop_candidate) = by_alias.get(alias.as_str()) else { continue };
                        let mut server = hop_candidate.server.clone();
                        server.name = free_name(store, &hop_candidate.name);
                        server.group = group.clone();
                        server.jump_server_id = previous.clone();
                        let Ok(id) = store.save(server.clone()) else { continue };
                        report.created.push(server.name);
                        made.insert(alias.clone(), id.clone());
                        id
                    }
                },
                Hop::Spec { user, host, port } => {
                    let key = (user.clone(), host.clone(), *port);
                    match specs.get(&key) {
                        Some(id) => id.clone(),
                        None => {
                            let mut server = Server::new(
                                &free_name(store, &spec_name(user, host, *port)),
                                host,
                                user,
                                AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None },
                            );
                            server.port = *port;
                            server.group = group.clone();
                            server.jump_server_id = previous.clone();
                            let Ok(id) = store.save(server.clone()) else { continue };
                            report.created.push(server.name);
                            specs.insert(key, id.clone());
                            id
                        }
                    }
                }
            };
            previous = Some(id);
        }
        if candidate.name_taken && candidate.name == candidate.alias {
            continue;
        }
        if store.by_name(&candidate.name).is_some() {
            continue;
        }
        let mut server = candidate.server.clone();
        server.name = candidate.name.clone();
        server.group = group.clone();
        server.jump_server_id = previous;
        if let Ok(id) = store.save(server.clone()) {
            report.created.push(server.name);
            made.insert(alias.clone(), id);
        }
    }
    report
}

/// A field that differs between an imported Server and its `Host` now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub field: &'static str,
    pub from: String,
    pub to: String,
}

impl FieldChange {
    pub fn describe(&self) -> String {
        format!("{} {} \u{2192} {}", self.field, self.from, self.to)
    }
}

/// How the Server (imported from `candidate.alias`) differs from what the
/// config says now.
pub fn changes_for(server: &Server, candidate: &Candidate, store: &ServerStore) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    let mut note = |field: &'static str, from: String, to: String| {
        if from != to {
            changes.push(FieldChange { field, from, to });
        }
    };
    note("address", server.host.clone(), candidate.server.host.clone());
    note("port", server.port.to_string(), candidate.server.port.to_string());
    note("user", server.user.clone(), candidate.server.user.clone());
    note("sign-in", auth_text(&server.auth), auth_text(&candidate.server.auth));
    let jump_now = server.jump_server_id.as_deref().and_then(|id| store.get(id)).map(|s| s.name.clone());
    let jump_new = candidate.hops.last().map(Hop::describe);
    note("jump server", jump_now.unwrap_or_else(|| "none".into()), jump_new.unwrap_or_else(|| "none".into()));
    changes
}

fn auth_text(auth: &AuthMethod) -> String {
    match auth {
        AuthMethod::Key { path } => format!("key {path}"),
        AuthMethod::Password => "password".into(),
        AuthMethod::Agent { socket, preferred_key } => {
            let mut text = format!("agent {}", socket.describe());
            if let Some(key) = preferred_key {
                text.push_str(&format!(" ({key})"));
            }
            text
        }
        AuthMethod::KeyboardInteractive => "keyboard-interactive".into(),
    }
}

/// Apply the chosen changes of a refresh to the Server (address, port,
/// user, sign-in; the Jump Server only when it can be found by name).
pub fn apply_changes(server: &mut Server, candidate: &Candidate, fields: &[&str], store: &ServerStore) {
    for field in fields {
        match *field {
            "address" => server.host = candidate.server.host.clone(),
            "port" => server.port = candidate.server.port,
            "user" => server.user = candidate.server.user.clone(),
            "sign-in" => server.auth = candidate.server.auth.clone(),
            "jump server" => {
                server.jump_server_id = match candidate.hops.last() {
                    Some(Hop::Existing { id, .. }) => Some(id.clone()),
                    Some(hop) => store.by_name(&hop.describe()).map(|s| s.id.clone()),
                    None => None,
                };
            }
            _ => {}
        }
    }
}

/// One Server of the app imported earlier, for the refresh view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshRow {
    /// Fields differing from the config now.
    Changed { server_id: String, name: String, changes: Vec<FieldChange> },
    /// Same as the config.
    Same { server_id: String, name: String },
    /// The alias is no longer in the config; the Server is kept.
    Gone { server_id: String, name: String },
}

/// Compare every imported Server with the config; aliases not yet imported
/// are the `new` candidates.
pub fn refresh(
    store: &ServerStore,
    all: &[Candidate],
) -> (Vec<RefreshRow>, Vec<Candidate>) {
    let by_alias: HashMap<&str, &Candidate> = all.iter().map(|c| (c.alias.as_str(), c)).collect();
    let mut rows = Vec::new();
    let mut imported: HashSet<&str> = HashSet::new();
    for server in &store.servers {
        let Some(Origin::SshConfig { alias }) = &server.origin else { continue };
        imported.insert(alias.as_str());
        let row = match by_alias.get(alias.as_str()) {
            None => RefreshRow::Gone { server_id: server.id.clone(), name: server.name.clone() },
            Some(candidate) => {
                let changes = changes_for(server, candidate, store);
                if changes.is_empty() {
                    RefreshRow::Same { server_id: server.id.clone(), name: server.name.clone() }
                } else {
                    RefreshRow::Changed { server_id: server.id.clone(), name: server.name.clone(), changes }
                }
            }
        };
        rows.push(row);
    }
    let new = all.iter().filter(|c| !imported.contains(c.alias.as_str())).cloned().collect();
    (rows, new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MapFs {
        files: HashMap<String, String>,
    }

    impl MapFs {
        fn with(files: &[(&str, &str)]) -> Self {
            Self { files: files.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect() }
        }
    }

    impl ConfigFs for MapFs {
        fn read(&self, path: &str) -> Option<String> {
            self.files.get(path).cloned()
        }
        fn list(&self, dir: &str) -> Vec<String> {
            let mut names: Vec<String> = self
                .files
                .keys()
                .filter_map(|p| p.strip_prefix(&format!("{dir}/")).filter(|r| !r.contains('/')).map(str::to_string))
                .collect();
            names.sort();
            names
        }
        fn exists(&self, path: &str) -> bool {
            self.files.contains_key(path)
        }
    }

    fn env() -> Env {
        Env { home: "/home/me".into(), local_user: "me".into() }
    }

    fn parse(config: &str, extra: &[(&str, &str)]) -> (ParsedConfig, MapFs) {
        let mut files: Vec<(&str, &str)> = extra.to_vec();
        files.push(("/home/me/.ssh/config", config));
        let fs = MapFs::with(&files);
        (read_config(&fs, &env()).unwrap(), fs)
    }

    fn host<'a>(parsed: &'a ParsedConfig, alias: &str) -> &'a ConfigHost {
        parsed.hosts.iter().find(|h| h.alias == alias).unwrap()
    }

    #[test]
    fn lines_split_in_every_form_ssh_accepts() {
        assert_eq!(tokenize_line("Port 22"), Some(("port".into(), vec!["22".into()])));
        assert_eq!(tokenize_line("  HostName=example.com"), Some(("hostname".into(), vec!["example.com".into()])));
        assert_eq!(tokenize_line("User = deploy"), Some(("user".into(), vec!["deploy".into()])));
        assert_eq!(
            tokenize_line(r#"IdentityFile "/path with space/key""#),
            Some(("identityfile".into(), vec!["/path with space/key".into()]))
        );
        assert_eq!(tokenize_line("Host a b  c"), Some(("host".into(), vec!["a".into(), "b".into(), "c".into()])));
        assert_eq!(tokenize_line("User ops # the ops user"), Some(("user".into(), vec!["ops".into()])));
        assert_eq!(tokenize_line("# comment"), None);
        assert_eq!(tokenize_line("   "), None);
    }

    #[test]
    fn the_first_value_wins_and_host_star_fills_in_what_is_missing() {
        let (parsed, _) = parse(
            "Host web1\n  HostName 10.0.0.5\n  User deploy\n\nHost web*\n  User other\n  Port 2200\n\nHost *\n  User ops\n  Port 22\n",
            &[],
        );
        let web1 = host(&parsed, "web1");
        assert_eq!(web1.host_name.as_deref(), Some("10.0.0.5"));
        assert_eq!(web1.user.as_deref(), Some("deploy"));
        assert_eq!(web1.port, Some(2200));
    }

    #[test]
    fn host_star_at_the_end_gives_a_user_to_hosts_without_one() {
        let (parsed, _) = parse("Host web1\n  HostName 10.0.0.5\nHost *\n  User ops\n", &[]);
        assert_eq!(host(&parsed, "web1").user.as_deref(), Some("ops"));
    }

    #[test]
    fn options_before_any_host_line_apply_to_every_host() {
        let (parsed, _) = parse("User first\nHost a\n  HostName x\n", &[]);
        assert_eq!(host(&parsed, "a").user.as_deref(), Some("first"));
    }

    #[test]
    fn negated_patterns_exclude_a_host() {
        let (parsed, _) = parse("Host web1 web2\n  HostName a\nHost * !web2\n  User ops\n", &[]);
        assert_eq!(host(&parsed, "web1").user.as_deref(), Some("ops"));
        assert_eq!(host(&parsed, "web2").user, None);
    }

    #[test]
    fn a_question_mark_matches_one_character() {
        let (parsed, _) = parse("Host web1 web10\n  HostName a\nHost web?\n  User one\n", &[]);
        assert_eq!(host(&parsed, "web1").user.as_deref(), Some("one"));
        assert_eq!(host(&parsed, "web10").user, None);
    }

    #[test]
    fn pattern_only_hosts_and_match_blocks_are_skipped_and_give_nothing() {
        let (parsed, _) = parse(
            "Host *.corp\n  User corp\nMatch host web1 exec \"true\"\n  User matched\nHost web1\n  HostName a\n",
            &[],
        );
        assert_eq!(parsed.hosts.len(), 1);
        assert_eq!(host(&parsed, "web1").user, None);
        let whats: Vec<&str> = parsed.skipped.iter().map(|s| s.what.as_str()).collect();
        assert_eq!(whats, vec!["Host *.corp", "Match host web1 exec true"]);
    }

    #[test]
    fn include_reads_relative_paths_from_ssh_dir_with_globs() {
        let (parsed, _) = parse(
            "Include conf.d/*.conf\nHost web1\n  HostName a\n",
            &[
                ("/home/me/.ssh/conf.d/a.conf", "Host db\n  HostName 10.0.1.9\n  Port 2222\n"),
                ("/home/me/.ssh/conf.d/b.conf", "Host cache\n  HostName 10.0.1.10\n"),
                ("/home/me/.ssh/conf.d/notes.txt", "Host ignored\n"),
            ],
        );
        let aliases: Vec<&str> = parsed.hosts.iter().map(|h| h.alias.as_str()).collect();
        assert_eq!(aliases, vec!["db", "cache", "web1"]);
        assert_eq!(host(&parsed, "db").port, Some(2222));
    }

    #[test]
    fn include_loops_stop() {
        let (parsed, _) = parse("Include loop\nHost a\n  HostName x\n", &[("/home/me/.ssh/loop", "Include loop\nHost b\n")]);
        assert!(parsed.hosts.iter().any(|h| h.alias == "a"));
    }

    #[test]
    fn tokens_and_tilde_are_expanded() {
        let (parsed, _) = parse(
            "Host web1\n  HostName %h.example.com\n  User deploy\n  IdentityFile ~/.ssh/%r_%h\n  IdentityFile %d/.ssh/second\n  IdentityAgent \"~/agent.sock\"\n",
            &[],
        );
        let web1 = host(&parsed, "web1");
        assert_eq!(web1.host_name.as_deref(), Some("web1.example.com"));
        assert_eq!(web1.identity_files, vec!["/home/me/.ssh/deploy_web1.example.com", "/home/me/.ssh/second"]);
        assert_eq!(web1.identity_agent.as_deref(), Some("/home/me/agent.sock"));
        let (parsed, _) = parse("Host a\n  Hostname 100%%\n", &[]);
        assert_eq!(host(&parsed, "a").host_name.as_deref(), Some("100%"));
    }

    #[test]
    fn unsupported_options_are_named() {
        let (parsed, _) = parse("Host legacy\n  ProxyCommand ssh -W %h:%p gw\n  LocalForward 8080 localhost:80\n", &[]);
        assert_eq!(host(&parsed, "legacy").unsupported, vec!["ProxyCommand", "LocalForward"]);
    }

    fn list(config: &str, extra: &[(&str, &str)], store: &ServerStore) -> Vec<Candidate> {
        let (parsed, fs) = parse(config, extra);
        candidates(&parsed, &fs, &env(), store)
    }

    fn find<'a>(all: &'a [Candidate], alias: &str) -> &'a Candidate {
        all.iter().find(|c| c.alias == alias).unwrap()
    }

    #[test]
    fn a_host_becomes_a_server_with_its_key() {
        let all = list(
            "Host web1\n  HostName 10.0.0.5\n  User deploy\n  IdentityFile ~/.ssh/id_ed25519\nHost db\n  HostName 10.0.1.9\n  Port 2222\n",
            &[("/home/me/.ssh/id_ed25519", "")],
            &ServerStore::default(),
        );
        let web1 = &find(&all, "web1").server;
        assert_eq!((web1.host.as_str(), web1.port, web1.user.as_str()), ("10.0.0.5", 22, "deploy"));
        assert_eq!(web1.auth, AuthMethod::Key { path: "/home/me/.ssh/id_ed25519".into() });
        assert_eq!(web1.origin, Some(Origin::SshConfig { alias: "web1".into() }));
        let db = &find(&all, "db").server;
        assert_eq!((db.host.as_str(), db.port, db.user.as_str()), ("10.0.1.9", 2222, "me"));
        assert_eq!(db.auth, AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None });
    }

    #[test]
    fn identity_agent_gives_an_agent_sign_in_with_that_socket() {
        let one_password = AgentSocket::OnePassword.resolve().unwrap();
        let config = format!("Host web1\n  IdentityAgent \"{one_password}\"\n  IdentityFile ~/.ssh/web1.pub\nHost web2\n  IdentityAgent /tmp/a.sock\n");
        let all = list(&config, &[("/home/me/.ssh/web1.pub", "")], &ServerStore::default());
        assert_eq!(
            find(&all, "web1").server.auth,
            AuthMethod::Agent { socket: AgentSocket::OnePassword, preferred_key: Some("/home/me/.ssh/web1.pub".into()) }
        );
        assert_eq!(
            find(&all, "web2").server.auth,
            AuthMethod::Agent { socket: AgentSocket::Path("/tmp/a.sock".into()), preferred_key: None }
        );
    }

    #[test]
    fn identity_agent_pageant_means_pageant_and_a_pipe_path_is_kept() {
        let config = "Host a\n  IdentityAgent pageant\nHost b\n  IdentityAgent \"\\\\.\\pipe\\my-agent\"\n";
        let all = list(config, &[], &ServerStore::default());
        assert_eq!(find(&all, "a").server.auth, AuthMethod::Agent { socket: AgentSocket::Pageant, preferred_key: None });
        assert_eq!(
            find(&all, "b").server.auth,
            AuthMethod::Agent { socket: AgentSocket::Path(r"\\.\pipe\my-agent".into()), preferred_key: None }
        );
    }

    #[test]
    fn a_missing_key_file_warns_and_falls_back_to_the_agent() {
        let all = list("Host web1\n  IdentityFile ~/.ssh/missing_key\n", &[], &ServerStore::default());
        let web1 = find(&all, "web1");
        assert_eq!(web1.warnings, vec!["Key file not found"]);
        assert_eq!(web1.server.auth, AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None });
        assert!(web1.checked);
    }

    #[test]
    fn proxy_command_is_not_ticked() {
        let all = list("Host legacy\n  ProxyCommand ssh -W %h:%p gw\n", &[], &ServerStore::default());
        let legacy = find(&all, "legacy");
        assert!(!legacy.checked);
        assert_eq!(legacy.warnings, vec!["ProxyCommand is not supported"]);
    }

    #[test]
    fn a_taken_name_is_flagged_and_not_ticked() {
        let mut store = ServerStore::default();
        store.save(Server::new("web1", "1.1.1.1", "me", AuthMethod::Password)).unwrap();
        let all = list("Host web1\n  HostName 10.0.0.5\n", &[], &store);
        let web1 = find(&all, "web1");
        assert!(web1.name_taken && !web1.checked);
        assert!(web1.warnings.contains(&"Name already used".to_string()));
    }

    #[test]
    fn the_same_alias_imported_before_is_not_a_conflict() {
        let mut store = ServerStore::default();
        let mut imported = Server::new("web1", "1.1.1.1", "me", AuthMethod::Password);
        imported.origin = Some(Origin::SshConfig { alias: "web1".into() });
        store.save(imported).unwrap();
        let all = list("Host web1\n  HostName 10.0.0.5\n", &[], &store);
        assert!(!find(&all, "web1").name_taken);
    }

    #[test]
    fn a_taken_name_can_be_renamed_on_the_row_and_imported() {
        let mut store = ServerStore::default();
        store.save(Server::new("web1", "1.1.1.1", "me", AuthMethod::Password)).unwrap();
        let mut all = list("Host web1\n  HostName 10.0.0.5\n", &[], &store);
        // Importing without renaming leaves the existing Server alone.
        let report = apply_import(&mut store, &all, &["web1".into()], None);
        assert!(report.created.is_empty());
        assert_eq!(store.get("srv-1").unwrap().host, "1.1.1.1");
        all[0].name = "web1-cfg".into();
        let report = apply_import(&mut store, &all, &["web1".into()], None);
        assert_eq!(report.created, vec!["web1-cfg"]);
        assert_eq!(store.by_name("web1-cfg").unwrap().host, "10.0.0.5");
    }

    #[test]
    fn proxy_jump_to_a_config_alias_makes_both_servers() {
        let mut store = ServerStore::default();
        let all = list(
            "Host bastion\n  HostName gw.example.com\n  User ops\nHost db\n  HostName 10.0.1.9\n  ProxyJump bastion\n",
            &[],
            &store,
        );
        assert_eq!(find(&all, "db").hops, vec![Hop::Config { alias: "bastion".into() }]);
        let report = apply_import(&mut store, &all, &["db".into()], None);
        assert_eq!(report.created, vec!["bastion", "db"]);
        let db = store.by_name("db").unwrap();
        assert_eq!(store.get(db.jump_server_id.as_deref().unwrap()).unwrap().name, "bastion");
    }

    #[test]
    fn selecting_both_does_not_make_the_jump_server_twice() {
        let mut store = ServerStore::default();
        let all = list("Host bastion\n  HostName gw\nHost db\n  HostName d\n  ProxyJump bastion\n", &[], &store);
        let report = apply_import(&mut store, &all, &["bastion".into(), "db".into()], None);
        assert_eq!(report.created, vec!["bastion", "db"]);
        assert_eq!(store.servers.len(), 2);
    }

    #[test]
    fn proxy_jump_without_aliases_makes_a_server_per_level() {
        let mut store = ServerStore::default();
        let all = list("Host db\n  HostName 10.0.1.9\n  User app\n  ProxyJump ops@gw1.example.com,gw2:2200\n", &[], &store);
        assert_eq!(
            find(&all, "db").hops,
            vec![
                Hop::Spec { user: "ops".into(), host: "gw1.example.com".into(), port: 22 },
                Hop::Spec { user: "me".into(), host: "gw2".into(), port: 2200 },
            ]
        );
        apply_import(&mut store, &all, &["db".into()], None);
        let chain: Vec<String> = store.chain(&store.by_name("db").unwrap().id).unwrap().iter().map(|s| s.name.clone()).collect();
        assert_eq!(chain, vec!["ops@gw1.example.com", "me@gw2:2200", "db"]);
    }

    #[test]
    fn proxy_jump_uses_a_server_the_app_already_has() {
        let mut store = ServerStore::default();
        store.save(Server::new("bastion", "gw", "ops", AuthMethod::Password)).unwrap();
        let all = list("Host db\n  HostName d\n  ProxyJump bastion\n", &[], &store);
        let report = apply_import(&mut store, &all, &["db".into()], None);
        assert_eq!(report.created, vec!["db"]);
        assert_eq!(store.by_name("db").unwrap().jump_server_id.as_deref(), Some("srv-1"));
    }

    #[test]
    fn proxy_jump_none_means_no_jump_server() {
        let all = list("Host db\n  ProxyJump none\nHost *\n  ProxyJump gw\n", &[], &ServerStore::default());
        assert!(find(&all, "db").hops.is_empty());
    }

    #[test]
    fn jump_loops_and_long_chains_are_errors() {
        let all = list("Host a\n  ProxyJump b\nHost b\n  ProxyJump a\n", &[], &ServerStore::default());
        assert!(find(&all, "a").error.is_some());
        assert!(!find(&all, "a").checked);
        let long = "Host t\n  ProxyJump h1,h2,h3,h4,h5\n";
        let all = list(long, &[], &ServerStore::default());
        assert!(find(&all, "t").error.as_deref().unwrap().contains("More than 4"));
        let mut store = ServerStore::default();
        let report = apply_import(&mut store, &all, &["t".into()], None);
        assert!(report.created.is_empty() && store.servers.is_empty());
    }

    #[test]
    fn imported_servers_join_the_chosen_server_group() {
        let mut store = ServerStore::default();
        let all = list("Host bastion\n  HostName gw\nHost db\n  ProxyJump bastion\n  HostName d\n", &[], &store);
        apply_import(&mut store, &all, &["db".into()], Some(" prod "));
        assert!(store.servers.iter().all(|s| s.group.as_deref() == Some("prod")));
    }

    #[test]
    fn refresh_shows_what_changed_and_what_is_gone() {
        let mut store = ServerStore::default();
        let all = list("Host db\n  HostName 10.0.1.9\nHost old\n  HostName o\n", &[], &store);
        apply_import(&mut store, &all, &["db".into(), "old".into()], None);

        let now = list("Host db\n  HostName 10.0.1.9\n  Port 2222\nHost fresh\n  HostName f\n", &[], &store);
        let (rows, new) = refresh(&store, &now);
        let kinds: Vec<String> = rows
            .iter()
            .map(|r| match r {
                RefreshRow::Changed { name, changes, .. } => format!("{name}: {}", changes[0].describe()),
                RefreshRow::Same { name, .. } => format!("{name}: same"),
                RefreshRow::Gone { name, .. } => format!("{name}: gone"),
            })
            .collect();
        assert_eq!(kinds, vec!["db: port 22 \u{2192} 2222", "old: gone"]);
        assert_eq!(new.iter().map(|c| c.alias.as_str()).collect::<Vec<_>>(), vec!["fresh"]);

        // Nothing changes until applied.
        assert_eq!(store.by_name("db").unwrap().port, 22);
        let db_id = store.by_name("db").unwrap().id.clone();
        let mut server = store.get(&db_id).unwrap().clone();
        apply_changes(&mut server, find(&now, "db"), &["port"], &store);
        assert_eq!(server.port, 2222);
        assert!(store.by_name("old").is_some());
    }
}
