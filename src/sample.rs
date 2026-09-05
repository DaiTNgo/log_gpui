use crate::model::{LogEntry, LogLevel};

pub fn generate_sample_logs() -> Vec<LogEntry> {
    let mut entries = Vec::new();
    let base_ts = 1757048400.0;

    let services = [
        ("gateway", "HTTP GET /api/v1/users/me 200 OK (12ms)"),
        ("auth-svc", "JWT token verified for user_id=4821"),
        ("db-pool", "Acquired connection from pool: active=14, idle=6"),
        ("redis", "CACHE HIT: key='session:usr:4821'"),
        ("billing", "Webhook received from Stripe: charge.succeeded"),
        ("worker", "Job #9921 processed in 142ms: send_welcome_email"),
        ("orders", "Order #48912 created: total=$129.99, items=3"),
        ("search", "Elasticsearch query executed: index='products', hits=42"),
        ("notifier", "Push notification dispatched to device token"),
        ("telemetry", "Prometheus metrics scraped by agent"),
    ];

    let events = [
        (LogLevel::Info, "Request completed successfully"),
        (LogLevel::Debug, "Debug context: trace_id=3a9f02c4b8, span_id=12a8"),
        (LogLevel::Verbose, "Payload size: 1024 bytes, compression=gzip"),
        (LogLevel::Warn, "Database query latency exceeded threshold: 245ms"),
        (LogLevel::Error, "Connection timeout to upstream redis cluster (10.0.1.4:6379)"),
        (LogLevel::Fatal, "Panic caught in worker goroutine: null pointer dereference"),
    ];

    let mut current_ts = base_ts;
    let mut index = 0;

    for round in 0..100 {
        for (i, (tag, default_msg)) in services.iter().enumerate() {
            index += 1;
            let time_step = 0.005 * ((round * 3 + i) % 7 + 1) as f64;
            current_ts += time_step;

            let hours = ((current_ts as u64 / 3600) % 24) as u32;
            let mins = ((current_ts as u64 / 60) % 60) as u32;
            let secs = (current_ts as u64 % 60) as u32;
            let micros = ((current_ts.fract()) * 1_000_000.0) as u32;
            let time_str = format!("{:02}:{:02}:{:02}.{:06}", hours, mins, secs, micros);

            let (level, payload) = if round % 15 == 0 && i == 5 {
                (events[5].0, format!("CRITICAL: {}", events[5].1))
            } else if round % 8 == 0 && i == 4 {
                (events[4].0, format!("ERROR: {}", events[4].1))
            } else if round % 5 == 0 && i == 3 {
                (events[3].0, format!("WARNING: {}", events[3].1))
            } else if i % 4 == 0 {
                (events[1].0, format!("{}: {}", default_msg, events[1].1))
            } else {
                (events[0].0, default_msg.to_string())
            };

            let raw_line = format!(
                "2026-09-05 {} [{}] [{}] {}",
                time_str,
                level.as_str(),
                tag,
                payload
            );

            let entry = LogEntry::new(
                index,
                current_ts,
                time_str,
                tag.to_string(),
                level,
                payload,
                raw_line,
            );

            entries.push(entry);
        }
    }

    entries
}
