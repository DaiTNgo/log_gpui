//! What the app knows about one source, from the events it sent: how many
//! times it connected again or ran, its last error, and when it last had a
//! line. Counted from the start of the app, never saved (L7: shown in the
//! status bar's Connection Details only).

use super::source::{BreakKind, RestartKind, SourceEvent, SourceStatus};
use std::time::SystemTime;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceHealth {
    /// Times a Remote File (or a Remote Command's connection) came back.
    pub reconnects: u32,
    /// Runs of a Remote Command started.
    pub runs: u32,
    pub last_error: Option<(SystemTime, String)>,
    /// When the last line arrived.
    pub last_line_at: Option<SystemTime>,
    /// How the last Run of a Remote Command ended, while it has not run again.
    pub exit_code: Option<u32>,
    /// The command line of the Run in progress, in the form it was sent to
    /// the Server (the `--since` form when resumed).
    pub run_command: Option<String>,
}

impl SourceHealth {
    /// Take note of what a source sent.
    pub fn observe(&mut self, event: &SourceEvent, now: SystemTime) {
        match event {
            SourceEvent::Append { lines, .. } if !lines.is_empty() => self.last_line_at = Some(now),
            SourceEvent::Break { kind } => match kind {
                BreakKind::Reconnected
                | BreakKind::Resumed(_)
                | BreakKind::Restarted(RestartKind::AfterDisconnect) => self.reconnects += 1,
                _ => {}
            },
            SourceEvent::Status(SourceStatus::Error(message)) => self.last_error = Some((now, message.clone())),
            SourceEvent::Status(SourceStatus::Exited(code)) => self.exit_code = *code,
            SourceEvent::Problem(message) => self.last_error = Some((now, message.clone())),
            SourceEvent::Run { command } => {
                self.runs += 1;
                self.run_command = Some(command.clone());
                self.exit_code = None;
            }
            _ => {}
        }
    }
}

/// The connection state in the status bar: the worst status of the members
/// with how many have it, and the latency of the slowest Server once they are
/// all connected: `Live · 42 ms`, `Reconnecting (1 of 3)`, `Exited (1 of 3)`.
pub fn connection_label(
    worst: &SourceStatus,
    count: usize,
    total: usize,
    slowest_latency: Option<std::time::Duration>,
) -> String {
    let base = match worst {
        SourceStatus::Live => "Live".to_string(),
        other if total > 1 => format!("{} ({count} of {total})", other.bar_label()),
        other => other.bar_label(),
    };
    match slowest_latency {
        Some(latency) => format!("{base} · {}", crate::remote::health::latency_text(Some(latency))),
        None => base,
    }
}
