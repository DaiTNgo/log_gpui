//! Logic: Connection Details, the popover opened from the connection state in
//! the status bar. It lists, for the open Remote Log Space, each Server's
//! connection health and the state of each Remote File and Remote Command on
//! it. Nothing here is saved.

use super::AppView;
use crate::live::{SourceCommand, SourceStatus};
use crate::remote::health::latency_text;
use crate::workspace::log_space::RemoteTarget;
use gpui::Context;
use std::time::{Duration, SystemTime};

/// What a member's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAction {
    /// A Remote File that is not live.
    Reconnect,
    /// A Remote Command.
    Restart,
    None,
}

#[derive(Debug, Clone)]
pub struct MemberDetails {
    pub file_id: String,
    pub name: String,
    pub status: String,
    /// Remote Command: it is a command (shown with its Run and exit code).
    pub is_command: bool,
    /// `10:02:11`, when the last line arrived.
    pub last_line: Option<String>,
    /// `Reconnects: 2` or `Runs: 3`.
    pub count: String,
    /// `10:02: Connection reset by peer`.
    pub last_error: Option<String>,
    /// `exit code 1`.
    pub exit: Option<String>,
    /// The command of the Run in progress.
    pub run_command: Option<String>,
    pub action: MemberAction,
}

#[derive(Debug, Clone)]
pub struct ServerDetails {
    pub server_id: String,
    pub name: String,
    /// `user@host:port`.
    pub address: String,
    /// `via bastion`, the Jump Servers outermost first.
    pub via: Option<String>,
    pub state: String,
    /// `42 ms (average 40 ms)`, or `—`.
    pub latency: String,
    /// `10:02:00`, when the session was made.
    pub connected_since: Option<String>,
    pub reconnects: u32,
    /// `every 30 s, 3 missed allowed`.
    pub keepalive: String,
    pub last_error: Option<String>,
    pub members: Vec<MemberDetails>,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionSnapshot {
    pub servers: Vec<ServerDetails>,
}

fn clock(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(time).format("%H:%M:%S").to_string()
}

fn short_clock(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(time).format("%H:%M").to_string()
}

fn seconds_text(d: Duration) -> String {
    format!("{} s", d.as_secs())
}

impl AppView {
    /// The Servers and members of the open Remote Log Space, as they are now.
    pub fn connection_snapshot(&self) -> Option<ConnectionSnapshot> {
        let space = self.active_space()?;
        let hub = self.remote.hub.as_ref();
        let mut servers: Vec<ServerDetails> = Vec::new();
        for file in space.ordered_files() {
            let Some(remote) = file.remote.as_ref() else { continue };
            let Some(server) = self.remote.servers.get(&remote.server_id) else { continue };
            let index = match servers.iter().position(|s| s.server_id == server.id) {
                Some(i) => i,
                None => {
                    let health = hub.map(|h| h.health().get(&server.id)).unwrap_or_default();
                    let latency = match (health.latency, health.latency_avg) {
                        (Some(last), Some(avg)) => format!("{} (average {})", latency_text(Some(last)), latency_text(Some(avg))),
                        _ => latency_text(None),
                    };
                    let (interval, missed) = if health.keepalive.0.is_zero() {
                        (Duration::from_secs(u64::from(server.keepalive_interval_secs)), server.keepalive_max_missed)
                    } else {
                        health.keepalive
                    };
                    let chain = self.remote.servers.chain(&server.id).unwrap_or_default();
                    let hops: Vec<String> = chain[..chain.len().saturating_sub(1)].iter().map(|s| s.name.clone()).collect();
                    servers.push(ServerDetails {
                        server_id: server.id.clone(),
                        name: server.name.clone(),
                        address: format!("{}@{}", server.user, server.address()),
                        via: (!hops.is_empty()).then(|| format!("via {}", hops.join(" → "))),
                        state: health.state.label().to_string(),
                        latency,
                        connected_since: health.connected_since.map(|since| clock(SystemTime::now() - since.elapsed())),
                        reconnects: health.reconnects,
                        keepalive: format!("every {}, {} missed allowed", seconds_text(interval), missed),
                        last_error: health.last_error.as_ref().map(|(at, message)| format!("{}: {message}", short_clock(*at))),
                        members: Vec::new(),
                    });
                    servers.len() - 1
                }
            };
            let source = self.live.sources.get(&file.file_id);
            let status = source.map(|s| s.status.clone());
            let health = source.map(|s| s.health.clone()).unwrap_or_default();
            let is_command = matches!(remote.target, RemoteTarget::Command { .. });
            let action = if is_command {
                MemberAction::Restart
            } else if matches!(
                status,
                Some(SourceStatus::Disconnected | SourceStatus::Error(_) | SourceStatus::NeedsSecret | SourceStatus::Reconnecting)
            ) {
                MemberAction::Reconnect
            } else {
                MemberAction::None
            };
            servers[index].members.push(MemberDetails {
                file_id: file.file_id.clone(),
                name: file.display_name.clone(),
                status: status.as_ref().map_or_else(|| "Not started".to_string(), SourceStatus::label),
                is_command,
                last_line: health.last_line_at.map(clock),
                count: if is_command { format!("Runs: {}", health.runs) } else { format!("Reconnects: {}", health.reconnects) },
                last_error: health.last_error.as_ref().map(|(at, message)| format!("{}: {message}", short_clock(*at))),
                exit: health.exit_code.map(|c| format!("exit code {c}")),
                run_command: if is_command { health.run_command.clone() } else { None },
                action,
            });
        }
        (!servers.is_empty()).then_some(ConnectionSnapshot { servers })
    }

    /// The latency of the slowest Server of the open Log Space, once every
    /// Server is connected and has been measured.
    pub fn slowest_server_latency(&self) -> Option<Duration> {
        let hub = self.remote.hub.as_ref()?;
        let space = self.active_space()?;
        let mut slowest: Option<Duration> = None;
        let mut any = false;
        let mut seen: Vec<&str> = Vec::new();
        for file in &space.files {
            let Some(remote) = file.remote.as_ref() else { continue };
            if seen.contains(&remote.server_id.as_str()) {
                continue;
            }
            seen.push(&remote.server_id);
            any = true;
            let health = hub.health().get(&remote.server_id);
            if health.state != crate::remote::ServerState::Connected {
                return None;
            }
            let latency = health.latency?;
            slowest = Some(slowest.map_or(latency, |s| s.max(latency)));
        }
        if any { slowest } else { None }
    }

    /// Reconnect the members of the open Log Space on a Server.
    pub fn reconnect_server_members(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .active_space()
            .map(|s| {
                s.files
                    .iter()
                    .filter(|f| f.remote.as_ref().is_some_and(|r| r.server_id == server_id))
                    .map(|f| f.file_id.clone())
                    .collect()
            })
            .unwrap_or_default();
        let needs_secret = ids
            .iter()
            .any(|id| self.live.sources.get(id).is_some_and(|s| s.status == SourceStatus::NeedsSecret));
        if needs_secret {
            self.open_secret_prompt(server_id, cx);
            return;
        }
        for id in ids {
            if let Some(source) = self.live.sources.get(&id) {
                source.handle.send(SourceCommand::Reconnect);
            }
        }
        cx.notify();
    }

    /// Reconnect one Remote File.
    pub fn reconnect_member(&mut self, file_id: &str, cx: &mut Context<Self>) {
        let server_id = self
            .active_space()
            .and_then(|s| s.file(file_id))
            .and_then(|f| f.remote.as_ref())
            .map(|r| r.server_id.clone());
        let Some(source) = self.live.sources.get(file_id) else { return };
        if source.status == SourceStatus::NeedsSecret {
            if let Some(server_id) = server_id {
                self.open_secret_prompt(&server_id, cx);
            }
            return;
        }
        source.handle.send(SourceCommand::Reconnect);
        cx.notify();
    }
}
