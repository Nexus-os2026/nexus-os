//! The loopback address of the local model runtime.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::ModelError;

/// A checked loopback address: `127.0.0.1` or `::1`, with a port. It is
/// made only by [`LoopbackEndpoint::parse`]. The name `localhost` becomes
/// `127.0.0.1`, so no name is ever resolved. Every connection is made to
/// this IP literal, and the address is checked again before each one.
///
/// There is no other way to make one than parsing: not a literal,
///
/// ```compile_fail,E0451
/// use nexus_kernel::inference::LoopbackEndpoint;
/// let endpoint = LoopbackEndpoint { addr: "192.0.2.1:9".parse().unwrap() };
/// ```
///
/// not a conversion from a URL,
///
/// ```compile_fail,E0277
/// use nexus_kernel::inference::LoopbackEndpoint;
/// let url = url::Url::parse("http://192.0.2.1:9/").unwrap();
/// let endpoint = LoopbackEndpoint::from(url);
/// ```
///
/// and not deserialization:
///
/// ```compile_fail,E0277
/// use nexus_kernel::inference::LoopbackEndpoint;
/// let endpoint: LoopbackEndpoint = serde_json::from_str("\"192.0.2.1:9\"").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopbackEndpoint {
    addr: SocketAddr,
}

impl LoopbackEndpoint {
    /// Parse `http://127.0.0.1:<port>`, `http://[::1]:<port>` or
    /// `http://localhost:<port>` (an optional trailing `/`). Any other host,
    /// a LAN, public or unspecified address, a mapped address or another
    /// name, is [`ModelError::NotLocal`]. Another scheme, user information,
    /// a port that is not written (an explicit `:80` is written), a path, a
    /// query or a fragment is [`ModelError::InvalidAddress`].
    pub fn parse(raw: &str) -> Result<Self, ModelError> {
        if raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(ModelError::InvalidAddress);
        }
        let url = url::Url::parse(raw).map_err(|_| ModelError::InvalidAddress)?;
        let ip = match url.host() {
            Some(url::Host::Ipv4(ip)) if ip == Ipv4Addr::LOCALHOST => IpAddr::V4(ip),
            Some(url::Host::Ipv6(ip)) if ip == Ipv6Addr::LOCALHOST => IpAddr::V6(ip),
            Some(url::Host::Domain(name)) if name.eq_ignore_ascii_case("localhost") => {
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            }
            _ => return Err(ModelError::NotLocal),
        };
        if url.scheme() != "http"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !matches!(url.path(), "" | "/")
        {
            return Err(ModelError::InvalidAddress);
        }
        // The port must be written. The URL parser drops a written default
        // port (`:80`), so the written text is read, and it must agree with
        // the parsed address that is connected to.
        let port = written_port(raw).ok_or(ModelError::InvalidAddress)?;
        if url.port_or_known_default() != Some(port) {
            return Err(ModelError::InvalidAddress);
        }
        Ok(Self {
            addr: SocketAddr::new(ip, port),
        })
    }

    /// The loopback socket address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

/// The port written in the authority of `http://<host>:<port>...`, if any:
/// the digits after the host (after `]` for an IPv6 literal). An authority
/// with user information (`@`, even empty) has none.
fn written_port(raw: &str) -> Option<u16> {
    let rest = raw
        .get(..7)
        .filter(|scheme| scheme.eq_ignore_ascii_case("http://"))
        .map(|_| &raw[7..])?;
    let authority = &rest[..rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len())];
    if authority.contains('@') {
        return None;
    }
    let after_host = match authority.strip_prefix('[') {
        Some(v6) => &v6[v6.find(']')? + 1..],
        None => &authority[authority.find(':')?..],
    };
    let digits = after_host.strip_prefix(':')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}
