//! Servers: where Remote Files live. Saved once for the whole app and shared
//! by every Project. A Server may go through another Server (a Jump Server),
//! up to four levels deep.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// How many Jump Servers a chain may hold.
pub const MAX_JUMPS: usize = 4;

/// How a Server is logged in to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    /// A private key file (OpenSSH format); the passphrase, if any, is in
    /// the Keychain.
    Key { path: String },
    /// A password kept in the Keychain when the user asked to remember it.
    Password,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    /// Name shown everywhere (`prod-web1`).
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub user: String,
    pub auth: AuthMethod,
    /// The Server this one is reached through.
    #[serde(default)]
    pub jump_server_id: Option<String>,
    /// Whether the password or passphrase is kept in the Keychain.
    #[serde(default)]
    pub remember_secret: bool,
    /// Seconds between keepalives (`KEEPALIVE_SECS_RANGE`).
    #[serde(default = "default_keepalive_secs")]
    pub keepalive_interval_secs: u32,
    /// Unanswered keepalives before the connection counts as lost
    /// (`KEEPALIVE_MISSED_RANGE`).
    #[serde(default = "default_keepalive_missed")]
    pub keepalive_max_missed: u32,
}

/// Allowed keepalive intervals, in seconds.
pub const KEEPALIVE_SECS_RANGE: std::ops::RangeInclusive<u32> = 5..=300;
/// Allowed numbers of missed keepalives.
pub const KEEPALIVE_MISSED_RANGE: std::ops::RangeInclusive<u32> = 1..=10;

fn default_port() -> u16 {
    22
}

fn default_keepalive_secs() -> u32 {
    30
}

fn default_keepalive_missed() -> u32 {
    3
}

impl Server {
    pub fn new(name: &str, host: &str, user: &str, auth: AuthMethod) -> Self {
        Self {
            id: String::new(),
            name: name.to_string(),
            host: host.to_string(),
            port: 22,
            user: user.to_string(),
            auth,
            jump_server_id: None,
            remember_secret: false,
            keepalive_interval_secs: default_keepalive_secs(),
            keepalive_max_missed: default_keepalive_missed(),
        }
    }

    /// `host:port`, the identity of a host key.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Account name of the secret in the Keychain.
    pub fn secret_account(&self) -> String {
        match self.auth {
            AuthMethod::Password => format!("{}:password", self.id),
            AuthMethod::Key { .. } => format!("{}:passphrase", self.id),
        }
    }
}

/// Why a Server cannot be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerError {
    BlankName,
    DuplicateName,
    BlankHost,
    BlankUser,
    KeepaliveInterval,
    KeepaliveMissed,
    UnknownJumpServer,
    JumpLoop,
    TooManyJumps,
    /// The typed port is not a number from 1 to 65535.
    InvalidPort,
    /// The Server is used by something and cannot be deleted; lists what.
    InUse(Vec<String>),
    NotFound,
}

impl ServerError {
    pub fn message(&self) -> String {
        match self {
            ServerError::BlankName => "Enter a name".into(),
            ServerError::DuplicateName => "A Server with this name already exists".into(),
            ServerError::BlankHost => "Enter an address".into(),
            ServerError::BlankUser => "Enter a user".into(),
            ServerError::KeepaliveInterval => {
                format!("Keepalive interval must be {} to {} seconds", KEEPALIVE_SECS_RANGE.start(), KEEPALIVE_SECS_RANGE.end())
            }
            ServerError::KeepaliveMissed => {
                format!("Missed keepalives must be {} to {}", KEEPALIVE_MISSED_RANGE.start(), KEEPALIVE_MISSED_RANGE.end())
            }
            ServerError::UnknownJumpServer => "The Jump Server does not exist".into(),
            ServerError::JumpLoop => "The Jump Server chain loops back on itself".into(),
            ServerError::TooManyJumps => format!("A chain may have at most {MAX_JUMPS} Jump Servers"),
            ServerError::InUse(users) => format!("Used by: {}", users.join(", ")),
            ServerError::NotFound => "Server not found".into(),
            ServerError::InvalidPort => "Port must be 1\u{2013}65535".into(),
        }
    }

    /// The field of the Server form the error is about; `None` for errors
    /// about the Server as a whole (in use, not found).
    pub fn field(&self) -> Option<ServerField> {
        match self {
            ServerError::BlankName | ServerError::DuplicateName => Some(ServerField::Name),
            ServerError::BlankHost => Some(ServerField::Host),
            ServerError::InvalidPort => Some(ServerField::Port),
            ServerError::BlankUser => Some(ServerField::User),
            ServerError::KeepaliveInterval => Some(ServerField::KeepaliveInterval),
            ServerError::KeepaliveMissed => Some(ServerField::KeepaliveMissed),
            ServerError::UnknownJumpServer | ServerError::JumpLoop | ServerError::TooManyJumps => {
                Some(ServerField::JumpServer)
            }
            ServerError::InUse(_) | ServerError::NotFound => None,
        }
    }
}

/// A field of the Server form, for showing an error next to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerField {
    Name,
    Host,
    Port,
    User,
    KeepaliveInterval,
    KeepaliveMissed,
    JumpServer,
}

/// The port typed in the Server form: empty means 22.
pub fn parse_port(text: &str) -> Result<u16, ServerError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(22);
    }
    match text.parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err(ServerError::InvalidPort),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerStore {
    pub servers: Vec<Server>,
    #[serde(default)]
    next_id: u64,
}

impl ServerStore {
    pub fn get(&self, id: &str) -> Option<&Server> {
        self.servers.iter().find(|s| s.id == id)
    }

    pub fn by_name(&self, name: &str) -> Option<&Server> {
        self.servers.iter().find(|s| s.name == name)
    }

    /// The Jump Servers of `id`, outermost first, then `id` itself last.
    /// `Err(JumpLoop)` when the chain loops.
    pub fn chain(&self, id: &str) -> Result<Vec<&Server>, ServerError> {
        let mut chain = Vec::new();
        let mut current = self.get(id).ok_or(ServerError::NotFound)?;
        loop {
            if chain.iter().any(|s: &&Server| s.id == current.id) {
                return Err(ServerError::JumpLoop);
            }
            chain.push(current);
            match &current.jump_server_id {
                Some(next) => current = self.get(next).ok_or(ServerError::UnknownJumpServer)?,
                None => break,
            }
        }
        chain.reverse();
        Ok(chain)
    }

    /// Check `server` as it would be saved, replacing `server.id` if it exists.
    pub fn validate(&self, server: &Server) -> Result<(), ServerError> {
        match self.validate_all(server).into_iter().next() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Every reason `server` cannot be saved, at most one per field, in
    /// form order.
    pub fn validate_all(&self, server: &Server) -> Vec<ServerError> {
        let mut errors = Vec::new();
        if server.name.trim().is_empty() {
            errors.push(ServerError::BlankName);
        } else if self.servers.iter().any(|s| s.name == server.name && s.id != server.id) {
            errors.push(ServerError::DuplicateName);
        }
        if server.host.trim().is_empty() {
            errors.push(ServerError::BlankHost);
        }
        if server.user.trim().is_empty() {
            errors.push(ServerError::BlankUser);
        }
        if !KEEPALIVE_SECS_RANGE.contains(&server.keepalive_interval_secs) {
            errors.push(ServerError::KeepaliveInterval);
        }
        if !KEEPALIVE_MISSED_RANGE.contains(&server.keepalive_max_missed) {
            errors.push(ServerError::KeepaliveMissed);
        }
        if let Err(e) = self.validate_chain(server) {
            errors.push(e);
        }
        errors
    }

    /// The Jump Server chain of `server` as it would be after saving, and
    /// of the Servers that go through it.
    fn validate_chain(&self, server: &Server) -> Result<(), ServerError> {
        // Walk the chain as it would be after saving.
        let mut seen = vec![server.id.as_str()];
        let mut depth = 0;
        let mut next = server.jump_server_id.as_deref();
        while let Some(id) = next {
            if seen.contains(&id) {
                return Err(ServerError::JumpLoop);
            }
            seen.push(id);
            depth += 1;
            if depth > MAX_JUMPS {
                return Err(ServerError::TooManyJumps);
            }
            let jump = self.get(id).ok_or(ServerError::UnknownJumpServer)?;
            next = jump.jump_server_id.as_deref();
        }
        // Servers that go through this one also grow longer by its chain.
        let own_depth = depth;
        for other in &self.servers {
            if other.id == server.id {
                continue;
            }
            // Depth of `other` through the modified `server`.
            let mut hops = 0;
            let mut cur = other.jump_server_id.as_deref();
            let mut guard = 0;
            while let Some(id) = cur {
                hops += 1;
                if id == server.id {
                    hops += own_depth;
                    if hops > MAX_JUMPS {
                        return Err(ServerError::TooManyJumps);
                    }
                    break;
                }
                cur = self.get(id).and_then(|s| s.jump_server_id.as_deref());
                guard += 1;
                if guard > 64 {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Add or replace (same `id`) a Server. A new one gets an id.
    pub fn save(&mut self, mut server: Server) -> Result<String, ServerError> {
        if server.id.is_empty() {
            self.next_id += 1;
            server.id = format!("srv-{}", self.next_id);
        } else if self.get(&server.id).is_none() {
            return Err(ServerError::NotFound);
        }
        self.validate(&server)?;
        let id = server.id.clone();
        match self.servers.iter_mut().find(|s| s.id == id) {
            Some(slot) => *slot = server,
            None => self.servers.push(server),
        }
        Ok(id)
    }

    /// Delete a Server unless another Server goes through it or `used_by`
    /// (what else uses it, supplied by the caller) is not empty.
    pub fn delete(&mut self, id: &str, used_by: Vec<String>) -> Result<Server, ServerError> {
        let mut users = used_by;
        for s in &self.servers {
            if s.jump_server_id.as_deref() == Some(id) {
                users.push(format!("Jump Server of {}", s.name));
            }
        }
        if !users.is_empty() {
            return Err(ServerError::InUse(users));
        }
        let pos = self.servers.iter().position(|s| s.id == id).ok_or(ServerError::NotFound)?;
        Ok(self.servers.remove(pos))
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }
}
