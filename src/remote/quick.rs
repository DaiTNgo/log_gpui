//! Quick Connect: one typed string stands for a Server and a path.
//! `[user@]host[:port]:<path>`, `ssh://[user@]host[:port]/<path>`, or
//! `<Server name>:<path>`; a host in `[]` may be IPv6; the path is absolute,
//! starts with `~/`, or is a glob.

use super::server::{AgentSocket, AuthMethod, Origin, Server, ServerStore};
use super::ssh_config::spec_name;

/// What the user typed does not have the shape of a Quick Connect string.
pub const EXPECTED: &str = "Expected user@host[:port]:/path";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickTarget {
    pub user: Option<String>,
    /// A host name, an address, or (with no user and no port) a Server name.
    pub host: String,
    pub port: Option<u16>,
    /// Absolute, or starting with `~/`.
    pub path: String,
}

fn valid_path(path: &str) -> bool {
    path.starts_with('/') || path.starts_with("~/")
}

/// Read a Quick Connect string; `Err` carries the message to show.
pub fn parse(input: &str) -> Result<QuickTarget, String> {
    let input = input.trim();
    let bad = || EXPECTED.to_string();
    if let Some(url) = input.strip_prefix("ssh://") {
        let slash = url.find('/').ok_or_else(bad)?;
        let (authority, path) = url.split_at(slash);
        let (user, hostport) = split_user(authority);
        let (host, port) = split_host_port(hostport).ok_or_else(bad)?;
        if host.is_empty() {
            return Err(bad());
        }
        // `ssh://host/~/x` writes a home-relative path.
        let path = path.strip_prefix("/~/").map_or_else(|| path.to_string(), |rest| format!("~/{rest}"));
        return Ok(QuickTarget { user, host, port, path });
    }
    let (user, rest) = split_user(input);
    // The host: `[v6]` or up to the first colon.
    let (host, after) = if let Some(inner) = rest.strip_prefix('[') {
        let (host, after) = inner.split_once(']').ok_or_else(bad)?;
        (host.to_string(), after.strip_prefix(':').ok_or_else(bad)?)
    } else {
        let colon = rest.find(':').ok_or_else(bad)?;
        (rest[..colon].to_string(), &rest[colon + 1..])
    };
    if host.is_empty() || host.contains(['/', '@']) {
        return Err(bad());
    }
    // A port is digits followed by another colon (`:2222:/path`).
    let (port, path) = match after.split_once(':') {
        Some((digits, path)) if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) => {
            (Some(digits.parse::<u16>().ok().filter(|p| *p > 0).ok_or_else(bad)?), path)
        }
        _ => (None, after),
    };
    if !valid_path(path) {
        return Err(bad());
    }
    Ok(QuickTarget { user, host, port, path: path.to_string() })
}

/// `user@rest` (the first `@` before any `/`), else `(None, text)`.
fn split_user(text: &str) -> (Option<String>, &str) {
    let first_slash = text.find('/').unwrap_or(text.len());
    match text[..first_slash].find('@') {
        Some(at) if at > 0 => (Some(text[..at].to_string()), &text[at + 1..]),
        _ => (None, text),
    }
}

/// `host`, `host:port`, `[v6]` or `[v6]:port`.
fn split_host_port(text: &str) -> Option<(String, Option<u16>)> {
    if let Some(inner) = text.strip_prefix('[') {
        let (host, after) = inner.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(p) => Some(p.parse().ok()?),
            None if after.is_empty() => None,
            None => return None,
        };
        return Some((host.to_string(), port));
    }
    match text.rsplit_once(':') {
        Some((host, port)) => Some((host.to_string(), Some(port.parse().ok()?))),
        None => Some((text.to_string(), None)),
    }
}

/// The Server a Quick Connect string stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Resolution {
    /// A Server the app has (its id).
    Existing(String),
    /// None matches: a Server to make, with a temporary id, signing in with
    /// the agent. It is saved only when a file is added with it.
    New(Server),
}

/// Prefix of the temporary id of a Server made by Quick Connect.
pub const TEMP_ID_PREFIX: &str = "quick-";

/// Pick the Server for `target`: a Server named like the host part (when no
/// user and no port were typed), else one with the same address, port and
/// user, else a new one.
pub fn resolve(store: &ServerStore, target: &QuickTarget, local_user: &str) -> Resolution {
    if let (None, None, Some(server)) = (&target.user, &target.port, store.by_name(&target.host)) {
        return Resolution::Existing(server.id.clone());
    }
    let user = target.user.clone().unwrap_or_else(|| local_user.to_string());
    let port = target.port.unwrap_or(22);
    if let Some(server) =
        store.servers.iter().find(|s| s.host.eq_ignore_ascii_case(&target.host) && s.port == port && s.user == user)
    {
        return Resolution::Existing(server.id.clone());
    }
    let mut server = Server::new(
        &spec_name(&user, &target.host, port),
        &target.host,
        &user,
        AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None },
    );
    server.port = port;
    server.origin = Some(Origin::QuickConnect);
    server.id = format!("{TEMP_ID_PREFIX}{}", store.servers.len() + 1);
    Resolution::New(server)
}

/// Whether a Server id is the temporary one of a Quick Connect.
pub fn is_temporary(id: &str) -> bool {
    id.starts_with(TEMP_ID_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(user: Option<&str>, host: &str, port: Option<u16>, path: &str) -> QuickTarget {
        QuickTarget { user: user.map(str::to_string), host: host.into(), port, path: path.into() }
    }

    #[test]
    fn every_form_of_the_string() {
        let cases = [
            ("web1:/var/log/app.log", t(None, "web1", None, "/var/log/app.log")),
            ("deploy@10.0.0.5:/app/app.log", t(Some("deploy"), "10.0.0.5", None, "/app/app.log")),
            ("ops@10.0.2.7:2222:/var/log/syslog", t(Some("ops"), "10.0.2.7", Some(2222), "/var/log/syslog")),
            ("10.0.2.7:2222:/var/log/syslog", t(None, "10.0.2.7", Some(2222), "/var/log/syslog")),
            ("ops@[fe80::1]:/var/log/x", t(Some("ops"), "fe80::1", None, "/var/log/x")),
            ("ops@[fe80::1]:2222:/var/log/x", t(Some("ops"), "fe80::1", Some(2222), "/var/log/x")),
            ("ops@host:~/logs/app.log", t(Some("ops"), "host", None, "~/logs/app.log")),
            ("ops@host:/var/log/*.log", t(Some("ops"), "host", None, "/var/log/*.log")),
            ("ssh://ops@host/var/log/x", t(Some("ops"), "host", None, "/var/log/x")),
            ("ssh://ops@host:2222/var/log/x", t(Some("ops"), "host", Some(2222), "/var/log/x")),
            ("ssh://host/var/log/x", t(None, "host", None, "/var/log/x")),
            ("ssh://ops@[::1]:2200/x", t(Some("ops"), "::1", Some(2200), "/x")),
            ("ssh://host/~/logs/x", t(None, "host", None, "~/logs/x")),
            ("  ops@host:/x  ", t(Some("ops"), "host", None, "/x")),
            // An `@` in the path belongs to the path.
            ("host:/var/log/a@b", t(None, "host", None, "/var/log/a@b")),
        ];
        for (input, expected) in cases {
            assert_eq!(parse(input), Ok(expected), "{input}");
        }
    }

    #[test]
    fn strings_of_the_wrong_shape_say_what_is_expected() {
        for input in [
            "10.0.2.7/var/log",
            "host",
            "host:",
            "host:relative/path",
            "host:2222",
            "@host:/x",
            ":/x",
            "ops@[fe80::1/x",
            "host:99999:/x",
            "host:0:/x",
            "ssh://host",
            "ssh://host:abc/x",
            "",
        ] {
            assert_eq!(parse(input), Err(EXPECTED.to_string()), "{input:?}");
        }
    }

    fn store() -> ServerStore {
        let mut store = ServerStore::default();
        let mut web1 = Server::new("web1", "10.0.0.5", "deploy", AuthMethod::Password);
        web1.port = 22;
        store.save(web1).unwrap();
        store
    }

    #[test]
    fn a_server_is_found_by_name() {
        let target = parse("web1:/var/log/app.log").unwrap();
        assert_eq!(resolve(&store(), &target, "me"), Resolution::Existing("srv-1".into()));
    }

    #[test]
    fn a_server_is_found_by_address_port_and_user() {
        let target = parse("deploy@10.0.0.5:/app/app.log").unwrap();
        assert_eq!(resolve(&store(), &target, "me"), Resolution::Existing("srv-1".into()));
        // Another user or port is another Server.
        let other_user = parse("root@10.0.0.5:/x").unwrap();
        assert!(matches!(resolve(&store(), &other_user, "me"), Resolution::New(_)));
        let other_port = parse("deploy@10.0.0.5:2222:/x").unwrap();
        assert!(matches!(resolve(&store(), &other_port, "me"), Resolution::New(_)));
    }

    #[test]
    fn a_missing_user_is_the_local_user_and_a_missing_port_is_22() {
        let target = parse("10.0.0.5:/x").unwrap();
        assert!(matches!(resolve(&store(), &target, "me"), Resolution::New(_)));
        let target = parse("10.0.0.5:/x").unwrap();
        assert_eq!(resolve(&store(), &target, "deploy"), Resolution::Existing("srv-1".into()));
    }

    #[test]
    fn a_new_server_signs_in_with_the_agent_and_is_named_after_its_address() {
        let target = parse("ops@10.0.2.7:2222:/var/log/syslog").unwrap();
        let Resolution::New(server) = resolve(&store(), &target, "me") else { panic!() };
        assert_eq!(server.name, "ops@10.0.2.7:2222");
        assert_eq!((server.host.as_str(), server.port, server.user.as_str()), ("10.0.2.7", 2222, "ops"));
        assert_eq!(server.auth, AuthMethod::Agent { socket: AgentSocket::System, preferred_key: None });
        assert_eq!(server.origin, Some(Origin::QuickConnect));
        assert!(is_temporary(&server.id));
        let target = parse("ops@10.0.2.7:/x").unwrap();
        let Resolution::New(server) = resolve(&store(), &target, "me") else { panic!() };
        assert_eq!(server.name, "ops@10.0.2.7");
    }

    #[test]
    fn a_name_with_a_user_or_port_is_not_looked_up_as_a_name() {
        let target = parse("ops@web1:/x").unwrap();
        assert!(matches!(resolve(&store(), &target, "me"), Resolution::New(_)));
    }
}
