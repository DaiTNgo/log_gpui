//! Host keys the user trusts, kept by the app apart from `~/.ssh/known_hosts`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// What a host key is, compared with the trusted one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyStatus {
    /// Never seen at this address: the user must confirm the fingerprint.
    Unknown,
    Trusted,
    /// Another key than the trusted one: the connection is refused.
    Changed { trusted: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownHosts {
    /// `host:port` to the SHA256 fingerprint of its key.
    hosts: BTreeMap<String, String>,
}

impl KnownHosts {
    pub fn check(&self, address: &str, fingerprint: &str) -> HostKeyStatus {
        match self.hosts.get(address) {
            None => HostKeyStatus::Unknown,
            Some(known) if known == fingerprint => HostKeyStatus::Trusted,
            Some(known) => HostKeyStatus::Changed { trusted: known.clone() },
        }
    }

    /// Trust `fingerprint` for `address`; replaces a key that changed.
    pub fn trust(&mut self, address: &str, fingerprint: &str) {
        self.hosts.insert(address.to_string(), fingerprint.to_string());
    }

    pub fn forget(&mut self, address: &str) {
        self.hosts.remove(address);
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
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
