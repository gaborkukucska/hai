//! START OF FILE hainet-noslop/src/hub/tor.rs
//!
//! Tor for the Home HUB:
//!
//! * [`send_line`]: one newline-terminated packet to `<onion>:9999` through the local SOCKS5 proxy.
//! * [`CreatorOnion`]: publishes the creator identity's onion with `ADD_ONION` on the Tor control
//!   connection, **without** `Flags=Detach`. The service therefore exists only while this Hub process
//!   holds the control connection: if the Hub stops, the creator onion disappears with it, and the
//!   phone can take it back without two owners announcing the same onion for long (single-owner fence).
//!   The connection is watched and the onion re-added if Tor restarts.
//!
//! Only well-formed v3 onion addresses are ever dialled (`is_valid_onion`), so nothing that reaches
//! the Hub's API can make it connect to an arbitrary host.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{watch, Mutex};
use tracing::{debug, info, warn};

/// The mesh port every NoSlop onion serves (`Constants.MESH_PORT`).
pub const MESH_PORT: u16 = 9999;

/// `[a-z2-7]{56}.onion`, case-insensitive.
pub fn is_valid_onion(onion: &str) -> bool {
    let o = onion.trim().to_ascii_lowercase();
    let Some(host) = o.strip_suffix(".onion") else { return false };
    host.len() == 56 && host.bytes().all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}

/// Sends one frame over Tor. Returns true when the whole line was written.
pub async fn send_line(socks: &str, onion: &str, line: &str, connect_timeout: Duration) -> bool {
    if !is_valid_onion(onion) {
        warn!("NOSLOP_HUB: refusing to dial an invalid onion address");
        return false;
    }
    let target = format!("{}:{}", onion.trim().to_ascii_lowercase(), MESH_PORT);
    match tokio::time::timeout(connect_timeout, tokio_socks::tcp::Socks5Stream::connect(socks, target.as_str())).await {
        Ok(Ok(mut stream)) => {
            let mut buf = line.as_bytes().to_vec();
            buf.push(b'\n');
            let ok = tokio::time::timeout(Duration::from_secs(60), async {
                stream.write_all(&buf).await?;
                stream.flush().await?;
                stream.shutdown().await
            }).await;
            matches!(ok, Ok(Ok(())))
        }
        Ok(Err(e)) => { debug!("NOSLOP_HUB: Tor connect to {} failed: {}", short(onion), e); false }
        Err(_) => { debug!("NOSLOP_HUB: Tor connect to {} timed out", short(onion)); false }
    }
}

pub fn short(s: &str) -> String { s.chars().take(12).collect() }

// --- Control port -----------------------------------------------------------------------------

/// Where the Tor control interface is (`HAINET_TOR_CONTROL`: `unix:/path` or `host:port`).
#[derive(Debug, Clone, PartialEq)]
pub enum ControlTarget {
    Unix(PathBuf),
    Tcp(String),
}

impl ControlTarget {
    pub fn parse(s: &str) -> Self {
        match s.strip_prefix("unix:") { Some(p) => ControlTarget::Unix(PathBuf::from(p)), None => ControlTarget::Tcp(s.to_string()) }
    }
}

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

struct Control {
    io: BufReader<Box<dyn Io>>,
}

impl Control {
    async fn connect(target: &ControlTarget) -> std::io::Result<Self> {
        let io: Box<dyn Io> = match target {
            #[cfg(unix)]
            ControlTarget::Unix(p) => Box::new(tokio::net::UnixStream::connect(p).await?),
            #[cfg(not(unix))]
            ControlTarget::Unix(_) => return Err(std::io::Error::other("unix sockets unsupported")),
            ControlTarget::Tcp(a) => Box::new(tokio::net::TcpStream::connect(a).await?),
        };
        Ok(Control { io: BufReader::new(io) })
    }

    /// Sends a command and reads its reply lines up to the final `NNN ` line.
    async fn cmd(&mut self, line: &str) -> std::io::Result<Vec<String>> {
        self.io.get_mut().write_all(format!("{}\r\n", line).as_bytes()).await?;
        self.io.get_mut().flush().await?;
        let mut out = Vec::new();
        loop {
            let mut l = String::new();
            let n = tokio::time::timeout(Duration::from_secs(30), self.io.read_line(&mut l)).await
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "tor control timeout"))??;
            if n == 0 { return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "tor control closed")); }
            let l = l.trim_end().to_string();
            let done = l.len() >= 4 && l.as_bytes()[3] == b' ';
            out.push(l);
            if done { return Ok(out); }
        }
    }

    async fn authenticate(&mut self, password: Option<&str>) -> std::io::Result<()> {
        let info = self.cmd("PROTOCOLINFO 1").await?;
        let joined = info.join("\n");
        let methods = joined.split("METHODS=").nth(1).map(|s| s.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default();
        let cookie_file = joined.split("COOKIEFILE=\"").nth(1).and_then(|s| s.split('"').next()).map(|s| s.replace("\\\\", "\\"));
        let auth_line = if methods.split(',').any(|m| m == "NULL") {
            "AUTHENTICATE".to_string()
        } else if methods.split(',').any(|m| m == "COOKIE") && cookie_file.is_some() {
            let cookie = tokio::fs::read(cookie_file.unwrap()).await?;
            format!("AUTHENTICATE {}", hex::encode(cookie))
        } else if methods.split(',').any(|m| m == "HASHEDPASSWORD") && password.is_some() {
            format!("AUTHENTICATE \"{}\"", password.unwrap().replace('\\', "\\\\").replace('"', "\\\""))
        } else {
            return Err(std::io::Error::other(format!("no usable Tor control auth method (offered: {})", methods)));
        };
        let r = self.cmd(&auth_line).await?;
        if r.last().map(|l| l.starts_with("250")).unwrap_or(false) { Ok(()) } else { Err(std::io::Error::other(format!("Tor control auth failed: {:?}", r))) }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OnionStatus {
    pub published: bool,
    pub service_id: Option<String>,
    pub error: Option<String>,
}

/// Publishes the creator onion while a seed is set (see module docs).
#[derive(Clone)]
pub struct CreatorOnion {
    desired: watch::Sender<Option<[u8; 32]>>,
    pub status: Arc<Mutex<OnionStatus>>,
}

impl CreatorOnion {
    /// Starts the watcher task. `local_port` is where Tor forwards the onion's port 9999.
    pub fn start(target: ControlTarget, password: Option<String>, local_port: u16) -> Self {
        let (tx, rx) = watch::channel::<Option<[u8; 32]>>(None);
        let status = Arc::new(Mutex::new(OnionStatus::default()));
        let st = status.clone();
        tokio::spawn(async move { Self::run(target, password, local_port, rx, st).await });
        CreatorOnion { desired: tx, status }
    }

    /// Publishes (Some) or withdraws (None) the creator onion.
    pub fn set(&self, seed: Option<[u8; 32]>) { let _ = self.desired.send(seed); }

    async fn run(target: ControlTarget, password: Option<String>, local_port: u16, mut rx: watch::Receiver<Option<[u8; 32]>>, status: Arc<Mutex<OnionStatus>>) {
        loop {
            let seed = *rx.borrow_and_update();
            let Some(seed) = seed else {
                *status.lock().await = OnionStatus::default();
                if rx.changed().await.is_err() { return; }
                continue;
            };
            match Self::publish(&target, password.as_deref(), local_port, &seed).await {
                Ok((mut ctl, service_id)) => {
                    info!("🧅 NOSLOP_HUB: creator onion {}.onion published (held by this Hub process)", short(&service_id));
                    *status.lock().await = OnionStatus { published: true, service_id: Some(service_id.clone()), error: None };
                    // Hold the control connection; a GETINFO every 60 s tells us when Tor went away.
                    loop {
                        tokio::select! {
                            changed = rx.changed() => {
                                if changed.is_err() { return; }
                                let _ = ctl.cmd(&format!("DEL_ONION {}", service_id)).await;
                                info!("🧅 NOSLOP_HUB: creator onion withdrawn");
                                break;
                            }
                            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                                if ctl.cmd("GETINFO version").await.is_err() {
                                    warn!("NOSLOP_HUB: Tor control connection lost; re-publishing the creator onion");
                                    break;
                                }
                            }
                        }
                    }
                    *status.lock().await = OnionStatus::default();
                }
                Err(e) => {
                    warn!("NOSLOP_HUB: could not publish the creator onion: {}", e);
                    *status.lock().await = OnionStatus { published: false, service_id: None, error: Some(e.to_string()) };
                    tokio::select! {
                        changed = rx.changed() => { if changed.is_err() { return; } }
                        _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                    }
                }
            }
        }
    }

    async fn publish(target: &ControlTarget, password: Option<&str>, local_port: u16, seed: &[u8; 32]) -> std::io::Result<(Control, String)> {
        let mut ctl = Control::connect(target).await?;
        ctl.authenticate(password).await?;
        let key = crate::keys::b64_encode(&crate::keys::tor_expanded_secret(seed));
        let reply = ctl.cmd(&format!("ADD_ONION ED25519-V3:{} Port={},127.0.0.1:{}", key, MESH_PORT, local_port)).await?;
        if let Some(id) = reply.iter().find_map(|l| l.split("ServiceID=").nth(1)).map(|s| s.trim().to_string()) {
            return Ok((ctl, id));
        }
        Err(std::io::Error::other(format!("ADD_ONION refused: {:?}", reply.last())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn onion_validation() {
        assert!(is_valid_onion("ovoezojfnst43rfm7xdm73w2qsibpznz7fiu5gizdplh4cynij3c2tad.onion"));
        assert!(is_valid_onion("OVOEZOJFNST43RFM7XDM73W2QSIBPZNZ7FIU5GIZDPLH4CYNIJ3C2TAD.onion"));
        assert!(!is_valid_onion("example.com"));
        assert!(!is_valid_onion("127.0.0.1"));
        assert!(!is_valid_onion("ovoezojfnst43rfm7xdm73w2qsibpznz7fiu5gizdplh4cynij3c2ta1.onion"));
        assert!(!is_valid_onion("short.onion"));
    }

    #[test]
    fn control_target_parse() {
        assert_eq!(ControlTarget::parse("unix:/run/tor/control"), ControlTarget::Unix("/run/tor/control".into()));
        assert_eq!(ControlTarget::parse("127.0.0.1:9051"), ControlTarget::Tcp("127.0.0.1:9051".into()));
    }
}
