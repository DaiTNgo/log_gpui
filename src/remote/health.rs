//! What the app knows about the connection to each Server: state, latency,
//! reconnects and the last error. Counted from the start of the app and never
//! saved (L7: shown in the status bar's Connection Details only).

use std::time::{Duration, Instant, SystemTime};

/// How many latency measurements the average runs over.
pub const LATENCY_SAMPLES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServerState {
    /// Never connected in this run, nothing wanted it yet.
    #[default]
    Idle,
    Connecting,
    Connected,
    /// Connecting again after a session was lost.
    Reconnecting,
    Disconnected,
    /// Waiting for the user (password, host key).
    Waiting,
    /// The agent is signing: waiting for a touch or an approval.
    WaitingForAgent,
}

impl ServerState {
    pub fn label(self) -> &'static str {
        match self {
            ServerState::Idle => "Not connected",
            ServerState::Connecting => "Connecting",
            ServerState::Connected => "Connected",
            ServerState::Reconnecting => "Reconnecting",
            ServerState::Disconnected => "Disconnected",
            ServerState::Waiting => "Waiting to connect",
            ServerState::WaitingForAgent => "Waiting for agent approval",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ServerHealth {
    pub state: ServerState,
    pub connected_since: Option<Instant>,
    /// The latest measurement.
    pub latency: Option<Duration>,
    /// Average of the last `LATENCY_SAMPLES` measurements.
    pub latency_avg: Option<Duration>,
    /// New sessions after an earlier one was lost, in this run of the app.
    pub reconnects: u32,
    pub last_error: Option<(SystemTime, String)>,
    /// (interval, missed allowed) of the session now or last used.
    pub keepalive: (Duration, u32),
    /// Names of the Jump Servers, outermost first.
    pub hops: Vec<String>,
    /// A session was made at least once in this run.
    pub ever_connected: bool,
    samples: Vec<Duration>,
}

impl ServerHealth {
    pub fn record_latency(&mut self, round_trip: Duration) {
        self.latency = Some(round_trip);
        self.samples.push(round_trip);
        if self.samples.len() > LATENCY_SAMPLES {
            self.samples.remove(0);
        }
        let total: Duration = self.samples.iter().sum();
        self.latency_avg = Some(total / self.samples.len() as u32);
    }

    /// A session ended; its measurements no longer describe the connection.
    pub fn clear_session(&mut self) {
        self.connected_since = None;
        self.clear_samples();
    }

    pub fn clear_samples(&mut self) {
        self.latency = None;
        self.latency_avg = None;
        self.samples.clear();
    }

    pub fn record_error(&mut self, message: impl Into<String>) {
        self.last_error = Some((SystemTime::now(), message.into()));
    }
}

/// `42 ms`, `1.2 s`; `—` when there is no measurement.
pub fn latency_text(latency: Option<Duration>) -> String {
    match latency {
        None => "—".into(),
        Some(d) if d < Duration::from_secs(1) => format!("{} ms", d.as_millis().max(1)),
        Some(d) => format!("{:.1} s", d.as_secs_f64()),
    }
}

/// The health of every Server, written from the connection tasks and read by
/// the app. A change is announced on `changes` (coalesced into one message).
#[derive(Clone)]
pub struct HealthBoard {
    map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, ServerHealth>>>,
    changed: async_channel::Sender<()>,
    changes: async_channel::Receiver<()>,
}

impl HealthBoard {
    pub fn new() -> Self {
        let (changed, changes) = async_channel::bounded(1);
        Self { map: Default::default(), changed, changes }
    }

    /// A message arrives here after the board changed (several changes may
    /// share one message).
    pub fn changes(&self) -> async_channel::Receiver<()> {
        self.changes.clone()
    }

    pub fn get(&self, server_id: &str) -> ServerHealth {
        self.map.lock().unwrap().get(server_id).cloned().unwrap_or_default()
    }

    pub fn update(&self, server_id: &str, change: impl FnOnce(&mut ServerHealth)) {
        change(self.map.lock().unwrap().entry(server_id.to_string()).or_default());
        let _ = self.changed.try_send(());
    }
}

impl Default for HealthBoard {
    fn default() -> Self {
        Self::new()
    }
}
