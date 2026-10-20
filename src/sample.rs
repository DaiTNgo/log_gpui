use crate::model::{LogEntry, LogLevel};

/// The sample entries, without their stack trace lines.
pub fn generate_sample_logs() -> Vec<LogEntry> {
    generate_samples().into_iter().map(|(entry, _)| entry).collect()
}

/// Each sample entry with the stack trace lines that follow it in the file.
fn generate_samples() -> Vec<(LogEntry, Vec<String>)> {
    let mut entries = Vec::new();
    let base_ts = 1757048400.0; // Simulated timestamp

    let services = [
        ("API", "HTTP GET /v2/checkout/session - 200 OK (14ms)"),
        ("DB", "SELECT users.*, accounts.balance FROM users JOIN accounts ON users.id = accounts.user_id WHERE users.id = $1"),
        ("AUTH", "JWT token validated: sub='usr_8829', roles=['admin', 'editor'], exp=1757052000"),
        ("PAYMENT", "Stripe Charge initiated: amount=14900, currency='usd', customer='cus_N8x2A'"),
        ("CACHE", "Redis CLUSTER FAILOVER: node redis-cluster-02 promoted to master"),
        ("WORKER", "Async job execution: queue='high_priority', handler='GenerateInvoicePdf'"),
        ("GATEWAY", "Inbound TLS handshake completed: cipher=TLS_AES_256_GCM_SHA384, proto=h2"),
    ];

    let mut current_ts = base_ts;
    let mut index = 0;

    for round in 0..120 {
        for (i, (tag, default_msg)) in services.iter().enumerate() {
            index += 1;
            let time_step = 0.003 * ((round * 5 + i) % 9 + 1) as f64;
            current_ts += time_step;

            let hours = ((current_ts as u64 / 3600) % 24) as u32;
            let mins = ((current_ts as u64 / 60) % 60) as u32;
            let secs = (current_ts as u64 % 60) as u32;
            let millis = ((current_ts.fract()) * 1_000.0) as u32;
            let time_str = format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis);

            let req_id = format!("req-{:04x}{:04x}", round % 99, i * 17);
            let trace_id = format!("trace-{:06x}", round * 10 + i);

            // Create specific scenarios
            let (level, payload, sub_lines) = if round % 18 == 0 && i == 0 {
                // Connection timeout in API
                (
                    LogLevel::Error,
                    "Connection timeout to upstream cluster (10.0.1.4:8080) after 5000ms".to_string(),
                    vec![
                        "java.net.SocketTimeoutException: Read timed out".to_string(),
                        "    at com.cloud.api.GatewayClient.execute(GatewayClient.java:142)".to_string(),
                        "    at com.cloud.api.Router.handleRequest(Router.java:88)".to_string(),
                        "    at io.netty.channel.AbstractChannelHandlerContext.fireChannelRead(Context.java:379)".to_string(),
                    ],
                )
            } else if round % 12 == 0 && i == 1 {
                // Database lock timeout
                (
                    LogLevel::Error,
                    "Deadlock detected: could not acquire exclusive row lock on pg_catalog.tx_log".to_string(),
                    vec![
                        "org.postgresql.util.PSQLException: ERROR: deadlock detected".to_string(),
                        "    Detail: Process 1428 waits for ShareLock on transaction 88192".to_string(),
                    ],
                )
            } else if round % 7 == 0 && i == 2 {
                // Warning
                (
                    LogLevel::Warn,
                    "Token expired: refresh requested from client IP 192.168.1.104".to_string(),
                    Vec::new(),
                )
            } else if round % 9 == 0 && i == 4 {
                // Cache warning
                (
                    LogLevel::Warn,
                    "Slow response from Redis master: latency 428ms exceeds p99 threshold (50ms)".to_string(),
                    Vec::new(),
                )
            } else if round % 25 == 0 && i == 3 {
                // Important bookmark
                (
                    LogLevel::Info,
                    "Payment confirmed: charge_id='ch_9921a8' settled with gateway".to_string(),
                    Vec::new(),
                )
            } else if round % 14 == 0 && i == 5 {
                (
                    LogLevel::Debug,
                    "Worker queue status: pending=48, active=6, processed_per_sec=142".to_string(),
                    Vec::new(),
                )
            } else if round % 11 == 0 {
                (
                    LogLevel::Debug,
                    format!("Query cache stats: hit_rate=98.4%, tag={}", tag),
                    Vec::new(),
                )
            } else {
                (
                    LogLevel::Info,
                    default_msg.to_string(),
                    Vec::new(),
                )
            };

            let raw_line = format!(
                "{} [{}] [{}] [req={}] {}",
                time_str,
                level.as_str(),
                tag,
                req_id,
                payload
            );

            let mut entry = LogEntry::new(
                index,
                current_ts,
                time_str,
                tag.to_string(),
                level,
                payload,
                raw_line,
            );

            entry.request_id = Some(req_id.clone());
            entry.trace_id = Some(trace_id);


            entries.push((entry, sub_lines));
        }
    }

    entries
}

/// The sample logs as text lines, in the standard timestamped format, for the
/// demo Log Space.
pub fn generate_sample_text() -> String {
    use chrono::DateTime;
    let mut out = String::new();
    for (entry, sub_lines) in generate_samples() {
        let secs = entry.timestamp.floor() as i64;
        let nanos = (entry.timestamp.fract() * 1_000_000_000.0) as u32;
        let stamp = DateTime::from_timestamp(secs, nanos)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
            .unwrap_or_else(|| entry.time_str.to_string());
        out.push_str(&format!(
            "{} [{}] [{}] {}\n",
            stamp,
            entry.level.as_str(),
            entry.tag,
            entry.payload
        ));
        for line in &sub_lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
