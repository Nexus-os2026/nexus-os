//! The governed browser's only way out: a session-owned proxy on the
//! loopback interface.
//!
//! The browser is launched with this proxy and with no implicit bypass, so
//! every connection it makes (pages, subresources, redirects, popups it is
//! not allowed to open anyway) arrives here. A connection is admitted only
//! to an origin the owner granted for the session: `CONNECT host:port` for
//! https origins, and an absolute-form request for an http origin only when
//! the grant names it. The destination's addresses are resolved once and
//! checked with the egress address policy, and the connection goes to
//! exactly those addresses (no DNS rebinding). Anything else is refused.
//!
//! The proxy has no credentials to protect and admits nothing the session's
//! grant does not; a local process that used it could reach only the
//! granted public origins, which it can reach directly anyway.

use crate::authority::AuthorityError;
use crate::egress::destination::{resolve_checked, Destination, Resolver};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_HEAD: usize = 16 * 1024;
const IDLE: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Most bytes one tunnel carries in each direction.
const MAX_TUNNEL: u64 = 256 * 1024 * 1024;
/// Client connections served at once; more are closed unserved.
const MAX_CONNECTIONS: usize = 64;
/// How often a blocked read looks at the stop flag: when the session ends,
/// every connection of its proxy ends within this.
const POLL: Duration = Duration::from_millis(250);

/// Which origins a session may reach.
pub(crate) struct OriginPolicy {
    /// Canonical `scheme://host:port`.
    pub origins: Vec<String>,
    /// Only test fixtures reach loopback servers.
    pub allow_private: bool,
    pub resolver: Arc<dyn Resolver>,
}

impl OriginPolicy {
    fn admit(&self, destination: &Destination) -> Option<Vec<std::net::SocketAddr>> {
        if !self.origins.contains(&destination.origin_text()) {
            return None;
        }
        resolve_checked(destination, self.allow_private, &*self.resolver).ok()
    }
}

/// A running proxy; it stops when dropped.
pub(crate) struct BrowserProxy {
    port: u16,
    stop: Arc<AtomicBool>,
    admitted: Arc<AtomicU32>,
    refused: Arc<AtomicU32>,
}

impl BrowserProxy {
    pub(crate) fn start(policy: OriginPolicy) -> Result<Self, AuthorityError> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|_| AuthorityError::Unavailable("the browser proxy cannot listen"))?;
        let port = listener
            .local_addr()
            .map_err(|_| AuthorityError::Unavailable("the browser proxy cannot listen"))?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|_| AuthorityError::Unavailable("the browser proxy cannot listen"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let admitted = Arc::new(AtomicU32::new(0));
        let refused = Arc::new(AtomicU32::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let policy = Arc::new(policy);
        {
            let (stop, admitted, refused) = (stop.clone(), admitted.clone(), refused.clone());
            std::thread::Builder::new()
                .name("nexus-p3-browser-proxy".into())
                .spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((client, _)) => {
                                if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                                    refused.fetch_add(1, Ordering::SeqCst);
                                    continue;
                                }
                                active.fetch_add(1, Ordering::SeqCst);
                                let (policy, stop) = (policy.clone(), stop.clone());
                                let (admitted, refused) = (admitted.clone(), refused.clone());
                                let active = active.clone();
                                std::thread::spawn(move || {
                                    let ok = serve(client, &policy, &stop);
                                    if ok {
                                        admitted.fetch_add(1, Ordering::SeqCst);
                                    } else {
                                        refused.fetch_add(1, Ordering::SeqCst);
                                    }
                                    active.fetch_sub(1, Ordering::SeqCst);
                                });
                            }
                            Err(_) => std::thread::sleep(Duration::from_millis(10)),
                        }
                    }
                })
                .map_err(|_| AuthorityError::Unavailable("the browser proxy cannot start"))?;
        }
        Ok(Self {
            port,
            stop,
            admitted,
            refused,
        })
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn admitted(&self) -> u32 {
        self.admitted.load(Ordering::SeqCst)
    }

    pub(crate) fn refused(&self) -> u32 {
        self.refused.load(Ordering::SeqCst)
    }
}

impl Drop for BrowserProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Whether a read failed only because nothing arrived within `POLL`.
fn idle(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

/// Read the request head (through the blank line); returns it and any bytes
/// read past it. It gives up when the proxy stops or the client idles.
fn read_head(client: &mut TcpStream, stop: &AtomicBool) -> Option<(String, Vec<u8>)> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 2048];
    let deadline = Instant::now() + IDLE;
    loop {
        let n = match client.read(&mut chunk) {
            Ok(n) => n,
            Err(error) if idle(&error) => {
                if stop.load(Ordering::SeqCst) || Instant::now() >= deadline {
                    return None;
                }
                continue;
            }
            Err(_) => return None,
        };
        if n == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buffer.split_off(end + 4);
            return Some((String::from_utf8(buffer).ok()?, rest));
        }
        if buffer.len() > MAX_HEAD {
            return None;
        }
    }
}

fn refuse(mut client: TcpStream) -> bool {
    let _ = client
        .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    false
}

fn connect(addresses: &[std::net::SocketAddr]) -> Option<TcpStream> {
    addresses
        .iter()
        .find_map(|address| TcpStream::connect_timeout(address, CONNECT_TIMEOUT).ok())
}

/// Serve one client connection; true if it was admitted.
fn serve(mut client: TcpStream, policy: &OriginPolicy, stop: &Arc<AtomicBool>) -> bool {
    let _ = client.set_read_timeout(Some(POLL));
    let _ = client.set_write_timeout(Some(IDLE));
    let Some((head, rest)) = read_head(&mut client, stop) else {
        return refuse(client);
    };
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (method, target, version) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    if !version.starts_with("HTTP/1.") || parts.next().is_some() {
        return refuse(client);
    }
    if method == "CONNECT" {
        let Ok(destination) = Destination::parse(&format!("https://{target}/")) else {
            return refuse(client);
        };
        let Some(addresses) = policy.admit(&destination) else {
            return refuse(client);
        };
        let Some(upstream) = connect(&addresses) else {
            return refuse(client);
        };
        if client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .is_err()
        {
            return false;
        }
        let mut upstream = upstream;
        if !rest.is_empty() && upstream.write_all(&rest).is_err() {
            return false;
        }
        tunnel(client, upstream, stop);
        return true;
    }
    // Plain http, absolute form, only to a granted http origin.
    if !target.starts_with("http://") {
        return refuse(client);
    }
    let Ok(destination) = Destination::parse(target) else {
        return refuse(client);
    };
    let Some(addresses) = policy.admit(&destination) else {
        return refuse(client);
    };
    let Some(mut upstream) = connect(&addresses) else {
        return refuse(client);
    };
    let path = match destination.url().query() {
        Some(query) => format!("{}?{query}", destination.url().path()),
        None => destination.url().path().to_string(),
    };
    let mut forwarded = format!("{method} {path} HTTP/1.1\r\n");
    for line in lines.filter(|l| !l.is_empty()) {
        let name = line
            .split(':')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "proxy-connection" | "proxy-authorization" | "connection" | "keep-alive"
        ) {
            continue;
        }
        forwarded.push_str(line);
        forwarded.push_str("\r\n");
    }
    forwarded.push_str("Connection: close\r\n\r\n");
    if upstream.write_all(forwarded.as_bytes()).is_err()
        || (!rest.is_empty() && upstream.write_all(&rest).is_err())
    {
        return false;
    }
    tunnel(client, upstream, stop);
    true
}

/// Copy both ways until either side closes, idles out, the cap, or the
/// proxy stops.
fn tunnel(client: TcpStream, upstream: TcpStream, stop: &Arc<AtomicBool>) {
    for stream in [&client, &upstream] {
        let _ = stream.set_read_timeout(Some(POLL));
        let _ = stream.set_write_timeout(Some(IDLE));
    }
    let (Ok(client_reader), Ok(upstream_reader)) = (client.try_clone(), upstream.try_clone())
    else {
        return;
    };
    let up_stop = stop.clone();
    let up = std::thread::spawn(move || copy(client_reader, upstream, &up_stop));
    copy(upstream_reader, client, stop);
    let _ = up.join();
}

fn copy(mut from: TcpStream, mut to: TcpStream, stop: &AtomicBool) {
    let mut buffer = [0u8; 16 * 1024];
    let mut total = 0u64;
    let mut quiet_since = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        match from.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                quiet_since = Instant::now();
                total += n as u64;
                if total > MAX_TUNNEL || to.write_all(&buffer[..n]).is_err() {
                    break;
                }
            }
            Err(error) if idle(&error) => {
                if quiet_since.elapsed() >= IDLE {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = to.shutdown(Shutdown::Write);
    let _ = from.shutdown(Shutdown::Read);
}
