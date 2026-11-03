//! The loopback address of the local model runtime.
//!
//! M2e-A1 interface stub: the inherited behaviour. The kernel's coding-run
//! transport takes any `url::Url` (`LocalEndpoint = url::Url`, F1); this
//! stub keeps whatever host address the URL names.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use super::ModelError;

/// The address of the local model runtime.
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
    /// Parse an HTTP address (stub: any host address, as the inherited
    /// transport accepts).
    pub fn parse(raw: &str) -> Result<Self, ModelError> {
        let url = crate::governed_http::http_url(raw).map_err(|_| ModelError::InvalidAddress)?;
        let ip = match url.host() {
            Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip),
            Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip),
            Some(url::Host::Domain(name)) if name.eq_ignore_ascii_case("localhost") => {
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            }
            _ => return Err(ModelError::InvalidAddress),
        };
        let port = url
            .port_or_known_default()
            .ok_or(ModelError::InvalidAddress)?;
        Ok(Self {
            addr: SocketAddr::new(ip, port),
        })
    }

    /// The socket address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}
