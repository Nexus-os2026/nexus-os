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
//!
//! It goes out only while the session may: once its run is cancelled or
//! its grant is revoked or expires, it admits, opens and writes nothing
//! more (data already handed to the kernel, or a write already under way,
//! may still complete), ending every tunnel. It keeps its port until the
//! session ends, closing every new connection unserved, so no other
//! process can take the port while the browser still uses it.

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
    /// Whether the session may still go out: false once its run is
    /// cancelled, its grants are revoked or expire, or the policy changes.
    pub live: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl OriginPolicy {
    fn admit(&self, destination: &Destination) -> Option<Vec<std::net::SocketAddr>> {
        if !self.origins.contains(&destination.origin_text()) {
            return None;
        }
        resolve_checked(destination, self.allow_private, &*self.resolver).ok()
    }
}

/// A running proxy; it releases its port when dropped.
pub(crate) struct BrowserProxy {
    port: u16,
    /// No more traffic.
    stop: Arc<AtomicBool>,
    /// The session ended: the port is released.
    closed: Arc<AtomicBool>,
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
        let closed = Arc::new(AtomicBool::new(false));
        let admitted = Arc::new(AtomicU32::new(0));
        let refused = Arc::new(AtomicU32::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let policy = Arc::new(policy);
        {
            let (stop, closed) = (stop.clone(), closed.clone());
            let (admitted, refused) = (admitted.clone(), refused.clone());
            std::thread::Builder::new()
                .name("nexus-p3-browser-proxy".into())
                .spawn(move || {
                    while !closed.load(Ordering::SeqCst) {
                        // A session that may no longer go out stops its
                        // proxy's traffic, and with it every open tunnel. A
                        // probe that fails counts as "no more": nothing ends
                        // this loop, which holds the port, but the session.
                        let live = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            (policy.live)()
                        }))
                        .unwrap_or(false);
                        if !live {
                            stop.store(true, Ordering::SeqCst);
                        }
                        match listener.accept() {
                            Ok((client, _)) => {
                                if stop.load(Ordering::SeqCst)
                                    || active.load(Ordering::SeqCst) >= MAX_CONNECTIONS
                                {
                                    refused.fetch_add(1, Ordering::SeqCst);
                                    drop(client);
                                    continue;
                                }
                                active.fetch_add(1, Ordering::SeqCst);
                                let (policy, stop) = (policy.clone(), stop.clone());
                                let (admitted, counted) = (admitted.clone(), refused.clone());
                                let serving = active.clone();
                                // A thread the system refuses leaves the
                                // connection unserved, not this loop ended.
                                let spawned = std::thread::Builder::new()
                                    .name("nexus-p3-browser-proxy-connection".into())
                                    .spawn(move || {
                                        let ok = serve(client, &policy, &stop);
                                        if ok {
                                            admitted.fetch_add(1, Ordering::SeqCst);
                                        } else {
                                            counted.fetch_add(1, Ordering::SeqCst);
                                        }
                                        serving.fetch_sub(1, Ordering::SeqCst);
                                    });
                                if spawned.is_err() {
                                    active.fetch_sub(1, Ordering::SeqCst);
                                    refused.fetch_add(1, Ordering::SeqCst);
                                }
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
            closed,
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
        self.closed.store(true, Ordering::SeqCst);
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

/// Whether a connection may take its next step out: the proxy has not
/// stopped and the session may still go out.
fn may_go_on(policy: &OriginPolicy, stop: &AtomicBool) -> bool {
    !stop.load(Ordering::SeqCst) && (policy.live)()
}

fn connect(addresses: &[std::net::SocketAddr], go_on: impl Fn() -> bool) -> Option<TcpStream> {
    for address in addresses {
        if !go_on() {
            return None;
        }
        if let Ok(stream) = TcpStream::connect_timeout(address, CONNECT_TIMEOUT) {
            return Some(stream);
        }
    }
    None
}

/// Serve one client connection; true if it was admitted.
pub(super) fn serve(
    mut client: TcpStream,
    policy: &Arc<OriginPolicy>,
    stop: &Arc<AtomicBool>,
) -> bool {
    let go_on = || may_go_on(policy, stop);
    let _ = client.set_read_timeout(Some(POLL));
    let _ = client.set_write_timeout(Some(IDLE));
    let Some((head, rest)) = read_head(&mut client, stop) else {
        return refuse(client);
    };
    if !go_on() {
        return refuse(client);
    }
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
        let Some(upstream) = connect(&addresses, go_on) else {
            return refuse(client);
        };
        if !go_on() {
            return refuse(client);
        }
        if client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .is_err()
        {
            return false;
        }
        let mut upstream = upstream;
        if !rest.is_empty() && (!go_on() || upstream.write_all(&rest).is_err()) {
            return false;
        }
        tunnel(client, upstream, policy, stop);
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
    let Some(mut upstream) = connect(&addresses, go_on) else {
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
    if !go_on() {
        return refuse(client);
    }
    if upstream.write_all(forwarded.as_bytes()).is_err()
        || (!rest.is_empty() && (!go_on() || upstream.write_all(&rest).is_err()))
    {
        return false;
    }
    tunnel(client, upstream, policy, stop);
    true
}

/// Copy both ways until either side closes, idles out, the cap, or the
/// proxy stops; nothing more is written once the session may no longer go
/// out.
fn tunnel(
    client: TcpStream,
    upstream: TcpStream,
    policy: &Arc<OriginPolicy>,
    stop: &Arc<AtomicBool>,
) {
    for stream in [&client, &upstream] {
        let _ = stream.set_read_timeout(Some(POLL));
        let _ = stream.set_write_timeout(Some(IDLE));
    }
    let (Ok(client_reader), Ok(upstream_reader)) = (client.try_clone(), upstream.try_clone())
    else {
        return;
    };
    let (up_policy, up_stop) = (policy.clone(), stop.clone());
    // A thread the system refuses ends the tunnel (both ends close), not the
    // connection's own thread.
    let Ok(up) = std::thread::Builder::new()
        .name("nexus-p3-browser-proxy-tunnel".into())
        .spawn(move || copy(client_reader, upstream, || may_go_on(&up_policy, &up_stop)))
    else {
        return;
    };
    copy(upstream_reader, client, || may_go_on(policy, stop));
    let _ = up.join();
}

fn copy(mut from: TcpStream, mut to: TcpStream, go_on: impl Fn() -> bool) {
    let mut buffer = [0u8; 16 * 1024];
    let mut total = 0u64;
    let mut quiet_since = Instant::now();
    while go_on() {
        match from.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                quiet_since = Instant::now();
                total += n as u64;
                if total > MAX_TUNNEL || !go_on() || to.write_all(&buffer[..n]).is_err() {
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
