//! Spike for the russh API: key with passphrase, jump through direct-tcpip,
//! SFTP read at an offset, exec. `ssh_spike <user> <key> <passphrase|-> <port> <remote file>`.

use russh::client::{self, Handle};
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key};
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

struct H;
impl client::Handler for H {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _k: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn auth(h: &mut Handle<H>, user: &str, key: &str, pass: Option<&str>) {
    let key = load_secret_key(key, pass).expect("key");
    let hash = h.best_supported_rsa_hash().await.unwrap().flatten();
    let ok = h
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .unwrap();
    assert!(ok.success(), "auth failed");
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (user, key, pass, port, file) = (&a[0], &a[1], &a[2], a[3].parse::<u16>().unwrap(), &a[4]);
    let pass = (pass != "-").then_some(pass.as_str());
    let cfg = Arc::new(client::Config::default());
    // Hop 1.
    let mut hop = client::connect(cfg.clone(), ("127.0.0.1", port), H).await.unwrap();
    auth(&mut hop, user, key, pass).await;
    // Hop 2 through hop 1.
    let ch = hop.channel_open_direct_tcpip("127.0.0.1", port as u32, "127.0.0.1", 0).await.unwrap();
    let mut target = client::connect_stream(cfg, ch.into_stream(), H).await.unwrap();
    auth(&mut target, user, key, pass).await;
    // SFTP at an offset.
    let ch = target.channel_open_session().await.unwrap();
    ch.request_subsystem(true, "sftp").await.unwrap();
    let sftp = SftpSession::new(ch.into_stream()).await.unwrap();
    let meta = sftp.metadata(file).await.unwrap();
    println!("size={:?}", meta.size);
    let mut f = sftp.open(file).await.unwrap();
    f.seek(std::io::SeekFrom::Start(5)).await.unwrap();
    let mut buf = vec![0u8; 10];
    let n = f.read(&mut buf).await.unwrap();
    println!("read {n}: {:?}", String::from_utf8_lossy(&buf[..n]));
    // exec.
    let mut ch = target.channel_open_session().await.unwrap();
    ch.exec(true, format!("stat -f '%i %z' {file} 2>/dev/null || stat -c '%i %s' {file}")).await.unwrap();
    while let Some(m) = ch.wait().await {
        if let russh::ChannelMsg::Data { data } = m {
            print!("stat: {}", String::from_utf8_lossy(&data));
        }
    }
}
