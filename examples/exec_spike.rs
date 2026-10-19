//! Spike for the russh exec stream: stdout and stderr apart, EOF on stdin,
//! exit status, `signal TERM`, and the round trip of a keepalive global
//! request. `exec_spike <user> <key> <port>`.

use russh::client::{self, Handle};
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key};
use russh::{ChannelMsg, Sig};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct H;
impl client::Handler for H {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _k: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn drain(label: &str, h: &Handle<H>, command: &str, signal_after: Option<Duration>) {
    println!("--- {label}: {command}");
    let started = Instant::now();
    let mut ch = h.channel_open_session().await.unwrap();
    ch.exec(true, command).await.unwrap();
    ch.eof().await.unwrap();
    let mut signalled = false;
    loop {
        let wait = ch.wait();
        let msg = match signal_after.filter(|_| !signalled) {
            Some(after) => match tokio::time::timeout(after.saturating_sub(started.elapsed()), wait).await {
                Ok(m) => m,
                Err(_) => {
                    println!("  [{:?}] send signal TERM", started.elapsed());
                    ch.signal(Sig::TERM).await.unwrap();
                    signalled = true;
                    continue;
                }
            },
            None => match tokio::time::timeout(Duration::from_secs(5), wait).await {
                Ok(m) => m,
                Err(_) => {
                    println!("  [{:?}] no message for 5 s: gave up", started.elapsed());
                    return;
                }
            },
        };
        let Some(msg) = msg else {
            println!("  [{:?}] channel ended", started.elapsed());
            return;
        };
        match msg {
            ChannelMsg::Data { data } => println!("  stdout {:?}", String::from_utf8_lossy(&data)),
            ChannelMsg::ExtendedData { data, ext } => {
                println!("  ext{ext} {:?}", String::from_utf8_lossy(&data))
            }
            ChannelMsg::ExitStatus { exit_status } => println!("  exit status {exit_status}"),
            ChannelMsg::ExitSignal { signal_name, .. } => println!("  exit signal {signal_name:?}"),
            other => println!("  {other:?}"),
        }
    }
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (user, key, port) = (&a[0], &a[1], a[2].parse::<u16>().unwrap());
    let cfg = Arc::new(client::Config::default());
    let mut h = client::connect(cfg, ("127.0.0.1", port), H).await.unwrap();
    let key = load_secret_key(key, None).unwrap();
    let hash = h.best_supported_rsa_hash().await.unwrap().flatten();
    assert!(h.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await.unwrap().success());

    drain("split streams", &h, "echo out; echo err 1>&2; exit 3", None).await;
    drain("stdin EOF", &h, "cat; echo done", None).await;
    drain("signal", &h, "sleep 30", Some(Duration::from_millis(500))).await;
    // Is the process gone after the signal?
    tokio::time::sleep(Duration::from_millis(300)).await;
    drain("leftover", &h, "pgrep -x sleep || echo none", None).await;

    for i in 0..3 {
        let t = Instant::now();
        let r = h.send_global_request("keepalive@openssh.com", &[], true).await;
        println!("keepalive round trip {i}: {:?} -> {:?}", t.elapsed(), r.map(|o| o.is_some()));
    }
    let t = Instant::now();
    h.send_ping().await.unwrap();
    println!("send_ping round trip: {:?}", t.elapsed());
}
