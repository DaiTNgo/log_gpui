//! What the operating system's own tools are called, for labels that name them.

/// The menu item that shows a file in the system file manager.
pub const REVEAL_IN_FILE_MANAGER: &str = if cfg!(target_os = "macos") {
    "Reveal in Finder"
} else if cfg!(windows) {
    "Show in Explorer"
} else {
    "Show in File Manager"
};

/// The checkbox that keeps a password or passphrase in the system's secret
/// store (`remote::Keychain`).
pub const REMEMBER_SECRET: &str = if cfg!(target_os = "macos") {
    "Remember in Keychain"
} else if cfg!(windows) {
    "Remember in Credential Manager"
} else {
    "Remember in the system keyring"
};
