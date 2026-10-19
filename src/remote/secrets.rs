//! Passwords and passphrases: kept in the system credential store (macOS
//! Keychain, Windows Credential Manager), never in a file of
//! the app.

use std::collections::HashMap;
use std::sync::Mutex;

/// Service name of the app's Keychain items.
pub const SERVICE: &str = "log_gpui";

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Option<String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, account: &str);
}

/// The macOS Keychain.
#[derive(Debug, Default, Clone, Copy)]
pub struct Keychain;

impl SecretStore for Keychain {
    fn get(&self, account: &str) -> Option<String> {
        keyring::Entry::new(SERVICE, account).ok()?.get_password().ok()
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, account)
            .and_then(|e| e.set_password(secret))
            .map_err(|e| e.to_string())
    }

    fn delete(&self, account: &str) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, account) {
            let _ = entry.delete_credential();
        }
    }
}

/// Secrets in memory, for tests.
#[derive(Debug, Default)]
pub struct MemorySecrets(Mutex<HashMap<String, String>>);

impl SecretStore for MemorySecrets {
    fn get(&self, account: &str) -> Option<String> {
        self.0.lock().unwrap().get(account).cloned()
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) {
        self.0.lock().unwrap().remove(account);
    }
}
