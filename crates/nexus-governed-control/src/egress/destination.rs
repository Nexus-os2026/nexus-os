//! Egress destinations: canonical URLs and the address policy.
//!
//! A URL is text; it becomes a destination only after it is parsed with the
//! WHATWG rules (the ones browsers use, so `0x7f.1`, `127.1` and
//! `2130706433` all become `127.0.0.1`), stripped of its fragment, and
//! checked: `https` (or `http`, only where a grant names it), no user
//! information, no whitespace or control characters, a host, a port. Every
//! address a destination resolves to is classified before anything connects
//! to it, and the connection is pinned to exactly the checked addresses.
//!
//! Not being in a private range does not make an address safe to reach:
//! where private addresses are not granted, every answer is also checked,
//! at each resolution, against this machine's own network boundary as it
//! is then ([`LocalNetworks`]: its interface addresses and the networks
//! directly connected to them). A public-looking address there is this
//! machine or its link, and is refused; so is everything when the boundary
//! cannot be read.

use crate::authority::ids::Digest;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use url::{Host, Url};

/// The longest URL accepted.
pub const MAX_URL: usize = 2048;
/// Most addresses one resolution may yield.
pub const MAX_ADDRESSES: usize = 16;

/// Why a URL or an address was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationError {
    TooLong,
    /// Not a URL, or it carries whitespace or control characters.
    Unparsable,
    /// Not `https` or `http`.
    Scheme,
    /// A user name or password in the URL.
    Userinfo,
    NoHost,
    /// A malformed host name.
    Host,
    /// The destination resolves to an address the policy refuses.
    NotPermitted,
    /// The destination does not resolve.
    Unresolvable,
    /// This machine's network boundary could not be read, so no address
    /// can be shown to be outside it.
    NoLocalBoundary,
}

impl DestinationError {
    pub fn as_str(self) -> &'static str {
        match self {
            DestinationError::TooLong => "url too long",
            DestinationError::Unparsable => "not a plain url",
            DestinationError::Scheme => "scheme not permitted",
            DestinationError::Userinfo => "user information in url",
            DestinationError::NoHost => "no host",
            DestinationError::Host => "malformed host",
            DestinationError::NotPermitted => "destination address not permitted",
            DestinationError::Unresolvable => "destination does not resolve",
            DestinationError::NoLocalBoundary => "the local network boundary cannot be read",
        }
    }
}

/// A destination host, canonical.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DestHost {
    /// Lower-case ASCII (internationalized names in punycode).
    Domain(String),
    Ip(IpAddr),
}

impl DestHost {
    /// The host as a grant names it (`[v6]` in brackets).
    pub fn text(&self) -> String {
        match self {
            DestHost::Domain(domain) => domain.clone(),
            DestHost::Ip(IpAddr::V4(v4)) => v4.to_string(),
            DestHost::Ip(IpAddr::V6(v6)) => format!("[{v6}]"),
        }
    }
}

/// A canonical request destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination {
    url: Url,
    scheme: &'static str,
    host: DestHost,
    port: u16,
}

impl Destination {
    /// Parse and check `text`.
    pub fn parse(text: &str) -> Result<Self, DestinationError> {
        if text.len() > MAX_URL {
            return Err(DestinationError::TooLong);
        }
        // The WHATWG parser silently drops tabs and newlines and trims
        // spaces; a URL that needs that is refused instead.
        if text.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(DestinationError::Unparsable);
        }
        let mut url = Url::parse(text).map_err(|_| DestinationError::Unparsable)?;
        let scheme = match url.scheme() {
            "https" => "https",
            "http" => "http",
            _ => return Err(DestinationError::Scheme),
        };
        if !url.username().is_empty() || url.password().is_some() {
            return Err(DestinationError::Userinfo);
        }
        url.set_fragment(None);
        let host = match url.host() {
            Some(Host::Domain(domain)) => DestHost::Domain(checked_domain(domain)?),
            Some(Host::Ipv4(v4)) => DestHost::Ip(IpAddr::V4(v4)),
            Some(Host::Ipv6(v6)) => DestHost::Ip(IpAddr::V6(v6)),
            None => return Err(DestinationError::NoHost),
        };
        let port = url
            .port_or_known_default()
            .ok_or(DestinationError::Scheme)?;
        if url.as_str().len() > MAX_URL {
            return Err(DestinationError::TooLong);
        }
        Ok(Self {
            url,
            scheme,
            host,
            port,
        })
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn scheme(&self) -> &'static str {
        self.scheme
    }

    pub fn host(&self) -> &DestHost {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `scheme://host:port`.
    pub fn origin_text(&self) -> String {
        format!("{}://{}:{}", self.scheme, self.host.text(), self.port)
    }

    /// The identity of the origin (what a commitment's target binds).
    pub fn origin_digest(&self) -> Digest {
        Digest::of(
            "nexus.p3.egress.origin.v1",
            &[
                self.scheme.as_bytes(),
                self.host.text().as_bytes(),
                &self.port.to_be_bytes(),
            ],
        )
    }

    pub fn same_origin(&self, other: &Destination) -> bool {
        self.scheme == other.scheme && self.host == other.host && self.port == other.port
    }

    /// Resolve a redirect's `Location` against this destination, checked
    /// like any other URL.
    pub fn join(&self, location: &str) -> Result<Destination, DestinationError> {
        if location.len() > MAX_URL || location.chars().any(|c| c.is_control()) {
            return Err(DestinationError::Unparsable);
        }
        let joined = self
            .url
            .join(location)
            .map_err(|_| DestinationError::Unparsable)?;
        Destination::parse(joined.as_str())
    }
}

/// A lower-case ASCII host name: labels of 1 to 63 letters, digits and
/// hyphens (punycode included), at most 253 characters, no trailing dot.
fn checked_domain(domain: &str) -> Result<String, DestinationError> {
    if domain.is_empty() || domain.len() > 253 || domain.ends_with('.') {
        return Err(DestinationError::Host);
    }
    for label in domain.split('.') {
        let ok = !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok {
            return Err(DestinationError::Host);
        }
    }
    Ok(domain.to_string())
}

/// What kind of address a destination resolved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressClass {
    /// Globally routable.
    Public,
    /// RFC 1918, shared address space, unique local, site-local.
    Private,
    Loopback,
    LinkLocal,
    /// Unspecified, multicast, broadcast, reserved, documentation,
    /// benchmarking, protocol assignments, deprecated transition forms:
    /// never a destination.
    Never,
}

impl AddressClass {
    /// Whether the policy lets a request reach it. Private, loopback and
    /// link-local addresses need a grant that names them; `Never` is never.
    pub fn permitted(self, allow_private: bool) -> bool {
        match self {
            AddressClass::Public => true,
            AddressClass::Private | AddressClass::Loopback | AddressClass::LinkLocal => {
                allow_private
            }
            AddressClass::Never => false,
        }
    }
}

/// Classify an address. IPv6 forms that embed an IPv4 address (mapped,
/// NAT64, 6to4) are classified by the address they embed.
pub fn classify(ip: IpAddr) -> AddressClass {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => classify_v6(v6),
    }
}

fn classify_v4(ip: Ipv4Addr) -> AddressClass {
    let [a, b, c, _] = ip.octets();
    match (a, b, c) {
        (0, _, _) => AddressClass::Never,
        (10, _, _) => AddressClass::Private,
        (100, 64..=127, _) => AddressClass::Private,
        (127, _, _) => AddressClass::Loopback,
        (169, 254, _) => AddressClass::LinkLocal,
        (172, 16..=31, _) => AddressClass::Private,
        (192, 0, 0) | (192, 0, 2) | (192, 88, 99) => AddressClass::Never,
        (192, 168, _) => AddressClass::Private,
        (198, 18..=19, _) => AddressClass::Never,
        (198, 51, 100) | (203, 0, 113) => AddressClass::Never,
        (224..=255, _, _) => AddressClass::Never,
        _ => AddressClass::Public,
    }
}

fn classify_v6(ip: Ipv6Addr) -> AddressClass {
    let s = ip.segments();
    if ip.is_unspecified() {
        return AddressClass::Never;
    }
    if ip.is_loopback() {
        return AddressClass::Loopback;
    }
    // ::ffff:a.b.c.d (mapped).
    if s[..5] == [0, 0, 0, 0, 0] && s[5] == 0xffff {
        return classify_v4(embedded_v4(s[6], s[7]));
    }
    // ::a.b.c.d (deprecated compatible) and ::ffff:0:a.b.c.d (translated).
    if s[..6] == [0, 0, 0, 0, 0, 0] || (s[..4] == [0, 0, 0, 0] && s[4] == 0xffff && s[5] == 0) {
        return AddressClass::Never;
    }
    // 64:ff9b::/96 (well-known NAT64 prefix).
    if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        return classify_v4(embedded_v4(s[6], s[7]));
    }
    // 2002::/16 (6to4).
    if s[0] == 0x2002 {
        return classify_v4(embedded_v4(s[1], s[2]));
    }
    match s[0] {
        // 64:ff9b:1::/48 (local NAT64).
        0x64 if s[1] == 0xff9b && s[2] == 1 => AddressClass::Never,
        // 100::/64 (discard).
        0x100 if s[1..4] == [0, 0, 0] => AddressClass::Never,
        // 2001::/23 (protocol assignments: Teredo, benchmarking, ORCHID)
        // and 2001:db8::/32 (documentation).
        0x2001 if s[1] < 0x200 || s[1] == 0xdb8 => AddressClass::Never,
        // 3fff::/20 (documentation).
        0x3fff if s[1] < 0x1000 => AddressClass::Never,
        x if x & 0xfe00 == 0xfc00 => AddressClass::Private,
        x if x & 0xffc0 == 0xfe80 => AddressClass::LinkLocal,
        x if x & 0xffc0 == 0xfec0 => AddressClass::Private,
        x if x & 0xff00 == 0xff00 => AddressClass::Never,
        _ => AddressClass::Public,
    }
}

fn embedded_v4(high: u16, low: u16) -> Ipv4Addr {
    let [a, b] = high.to_be_bytes();
    let [c, d] = low.to_be_bytes();
    Ipv4Addr::new(a, b, c, d)
}

/// This machine's side of the network: each entry an interface address
/// (or a point-to-point peer) with the prefix of the network directly
/// connected through it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalNetworks(pub Vec<(IpAddr, u8)>);

impl LocalNetworks {
    /// Whether `ip` is one of these addresses or in one of these networks.
    /// An IPv6 form that embeds an IPv4 address (mapped, NAT64, 6to4) is
    /// local when either it or the address it embeds is.
    pub fn contains(&self, ip: IpAddr) -> bool {
        let embedded = match ip {
            IpAddr::V6(v6) => embedded_in(v6).map(IpAddr::V4),
            IpAddr::V4(_) => None,
        };
        self.0.iter().any(|(network, length)| {
            in_network(ip, *network, *length)
                || embedded.is_some_and(|inner| in_network(inner, *network, *length))
        })
    }
}

fn in_network(ip: IpAddr, network: IpAddr, length: u8) -> bool {
    match (ip, network) {
        (IpAddr::V4(ip), IpAddr::V4(network)) => {
            let length = u32::from(length.min(32));
            let mask = u32::MAX.checked_shl(32 - length).unwrap_or(0);
            u32::from(ip) & mask == u32::from(network) & mask
        }
        (IpAddr::V6(ip), IpAddr::V6(network)) => {
            let length = u32::from(length.min(128));
            let mask = u128::MAX.checked_shl(128 - length).unwrap_or(0);
            u128::from(ip) & mask == u128::from(network) & mask
        }
        _ => false,
    }
}

/// The IPv4 address an IPv6 form embeds, if it is one that does.
fn embedded_in(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    // Mapped (::ffff:a.b.c.d) and the well-known NAT64 prefix.
    if (s[..5] == [0, 0, 0, 0, 0] && s[5] == 0xffff) || s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        Some(embedded_v4(s[6], s[7]))
    } else if s[0] == 0x2002 {
        Some(embedded_v4(s[1], s[2]))
    } else {
        None
    }
}

/// Name resolution, and this machine's network boundary as it is now:
/// both injectable for tests.
pub trait Resolver: Send + Sync {
    fn resolve(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>>;

    /// This machine's interface addresses and directly connected networks,
    /// read again at every resolution (the system's unless a test supplies
    /// a view). An error means the boundary cannot be established.
    fn local_networks(&self) -> std::io::Result<LocalNetworks> {
        super::interfaces::system()
    }
}

/// The system resolver.
pub struct SystemResolver;

impl Resolver for SystemResolver {
    fn resolve(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        (host, port).to_socket_addrs().map(|addrs| addrs.collect())
    }
}

/// Resolve `destination` once and check every address it yields; the
/// caller connects only to the returned addresses. A destination with any
/// refused address is refused entirely (a mixed answer is how rebinding
/// starts).
pub fn resolve_checked(
    destination: &Destination,
    allow_private: bool,
    resolver: &dyn Resolver,
) -> Result<Vec<SocketAddr>, DestinationError> {
    let mut addresses = match destination.host() {
        DestHost::Ip(ip) => vec![SocketAddr::new(*ip, destination.port())],
        DestHost::Domain(domain) => {
            // Names that mean this machine or its link, whatever DNS says.
            if domain == "localhost" || domain.ends_with(".localhost") {
                if !AddressClass::Loopback.permitted(allow_private) {
                    return Err(DestinationError::NotPermitted);
                }
            } else if (domain.ends_with(".local") || domain.ends_with(".home.arpa"))
                && !AddressClass::Private.permitted(allow_private)
            {
                return Err(DestinationError::NotPermitted);
            }
            resolver
                .resolve(domain, destination.port())
                .map_err(|_| DestinationError::Unresolvable)?
        }
    };
    addresses.sort();
    addresses.dedup();
    if addresses.is_empty() {
        return Err(DestinationError::Unresolvable);
    }
    // Every address is classified before any is set aside: a refused one
    // anywhere in the answer refuses it.
    if addresses
        .iter()
        .any(|address| !classify(address.ip()).permitted(allow_private))
    {
        return Err(DestinationError::NotPermitted);
    }
    // Where private addresses are not granted, an address of this machine,
    // or in a network directly connected to it, is refused however public
    // it looks: the boundary as it is now, read for this answer.
    if !allow_private {
        let local = resolver
            .local_networks()
            .map_err(|_| DestinationError::NoLocalBoundary)?;
        if addresses.iter().any(|address| local.contains(address.ip())) {
            return Err(DestinationError::NotPermitted);
        }
    }
    addresses.truncate(MAX_ADDRESSES);
    Ok(addresses)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Destination, DestinationError> {
        Destination::parse(text)
    }

    #[test]
    fn only_plain_https_and_http_urls_become_destinations() {
        let d = parse("https://Example.COM/a/b?q=1#frag").unwrap();
        assert_eq!(d.origin_text(), "https://example.com:443");
        assert_eq!(d.url().as_str(), "https://example.com/a/b?q=1");
        assert_eq!(parse("http://example.com:8080/").unwrap().port(), 8080);
        for refused in [
            "file:///etc/passwd",
            "data:text/html,hi",
            "javascript:alert(1)",
            "ftp://example.com/",
            "ws://example.com/",
            "wss://example.com/",
            "unix:/run/docker.sock",
            "chrome://settings",
            "about:blank",
        ] {
            assert_eq!(
                parse(refused).unwrap_err(),
                DestinationError::Scheme,
                "{refused}"
            );
        }
        assert_eq!(
            parse("https://user:pw@example.com/").unwrap_err(),
            DestinationError::Userinfo
        );
        assert_eq!(
            parse("https://example.com@evil.com/").unwrap_err(),
            DestinationError::Userinfo
        );
        assert_eq!(
            parse("https://exa\nmple.com/").unwrap_err(),
            DestinationError::Unparsable
        );
        assert_eq!(
            parse(" https://example.com/").unwrap_err(),
            DestinationError::Unparsable
        );
        assert_eq!(
            parse("https://example.com/\t").unwrap_err(),
            DestinationError::Unparsable
        );
        assert_eq!(
            parse("https://example.com./").unwrap_err(),
            DestinationError::Host
        );
        assert_eq!(
            parse(&format!("https://example.com/{}", "a".repeat(MAX_URL))).unwrap_err(),
            DestinationError::TooLong
        );
        assert!(parse("https://bad_host.example/").is_err());
    }

    #[test]
    fn host_tricks_resolve_to_what_they_really_are() {
        // Every spelling of loopback is loopback.
        for text in [
            "http://127.0.0.1/",
            "http://127.1/",
            "http://0x7f.1/",
            "http://0x7f000001/",
            "http://2130706433/",
            "http://0177.0.0.1/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:7f00:1]/",
            "http://[64:ff9b::7f00:1]/",
            "http://[2002:7f00:1::]/",
        ] {
            let d = parse(text).unwrap();
            let DestHost::Ip(ip) = d.host() else {
                panic!("{text} is an address")
            };
            assert_eq!(classify(*ip), AddressClass::Loopback, "{text}");
        }
        // A backslash is a path separator, not part of the host.
        let d = parse("https://evil.example\\@good.example/").unwrap();
        assert_eq!(d.host(), &DestHost::Domain("evil.example".into()));
        // Percent-encoding and international names are canonical.
        assert_eq!(
            parse("https://%65xample.com/").unwrap().host(),
            &DestHost::Domain("example.com".into())
        );
        assert_eq!(
            parse("https://bücher.example/").unwrap().host(),
            &DestHost::Domain("xn--bcher-kva.example".into())
        );
        // The default port is the same origin.
        assert!(parse("https://example.com:443/")
            .unwrap()
            .same_origin(&parse("https://example.com/").unwrap()));
        assert!(!parse("https://example.com:8443/")
            .unwrap()
            .same_origin(&parse("https://example.com/").unwrap()));
    }

    #[test]
    fn address_classes() {
        let v4 = |s: &str| classify(IpAddr::V4(s.parse().unwrap()));
        let v6 = |s: &str| classify(IpAddr::V6(s.parse().unwrap()));
        assert_eq!(v4("93.184.216.34"), AddressClass::Public);
        assert_eq!(v4("8.8.8.8"), AddressClass::Public);
        for private in [
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "100.64.0.1",
        ] {
            assert_eq!(v4(private), AddressClass::Private, "{private}");
        }
        assert_eq!(v4("172.32.0.1"), AddressClass::Public);
        assert_eq!(v4("169.254.169.254"), AddressClass::LinkLocal);
        for never in [
            "0.0.0.0",
            "0.1.2.3",
            "224.0.0.1",
            "239.255.255.250",
            "240.0.0.1",
            "255.255.255.255",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "198.18.0.1",
            "192.0.0.1",
            "192.88.99.1",
        ] {
            assert_eq!(v4(never), AddressClass::Never, "{never}");
        }
        assert_eq!(v6("2606:4700::1111"), AddressClass::Public);
        assert_eq!(v6("::1"), AddressClass::Loopback);
        assert_eq!(v6("::"), AddressClass::Never);
        assert_eq!(v6("fe80::1"), AddressClass::LinkLocal);
        assert_eq!(v6("fd00::1"), AddressClass::Private);
        assert_eq!(v6("fec0::1"), AddressClass::Private);
        assert_eq!(v6("ff02::1"), AddressClass::Never);
        assert_eq!(v6("2001:db8::1"), AddressClass::Never);
        assert_eq!(v6("2001:0:4136:e378::1"), AddressClass::Never, "Teredo");
        assert_eq!(v6("100::1"), AddressClass::Never);
        assert_eq!(v6("::127.0.0.1"), AddressClass::Never);
        assert_eq!(v6("::ffff:0:7f00:1"), AddressClass::Never);
        assert_eq!(v6("64:ff9b:1::1"), AddressClass::Never);
        assert_eq!(v6("::ffff:10.0.0.1"), AddressClass::Private);
        assert_eq!(v6("::ffff:8.8.8.8"), AddressClass::Public);
        assert_eq!(v6("64:ff9b::808:808"), AddressClass::Public);
        assert_eq!(v6("2002:a00:1::"), AddressClass::Private);
        assert!(AddressClass::Public.permitted(false));
        assert!(!AddressClass::Private.permitted(false));
        assert!(AddressClass::Loopback.permitted(true));
        assert!(!AddressClass::Never.permitted(true));
    }

    struct Fixed(Vec<&'static str>);
    impl Resolver for Fixed {
        fn resolve(&self, _host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
            Ok(self
                .0
                .iter()
                .map(|ip| SocketAddr::new(ip.parse().unwrap(), port))
                .collect())
        }
    }

    #[test]
    fn every_resolved_address_must_be_permitted() {
        let d = parse("https://rebind.example/").unwrap();
        assert_eq!(
            resolve_checked(&d, false, &Fixed(vec!["93.184.216.34"]))
                .unwrap()
                .len(),
            1
        );
        // One private answer among public ones refuses the destination.
        assert_eq!(
            resolve_checked(&d, false, &Fixed(vec!["93.184.216.34", "127.0.0.1"])).unwrap_err(),
            DestinationError::NotPermitted
        );
        assert_eq!(
            resolve_checked(&d, false, &Fixed(vec!["::ffff:169.254.169.254"])).unwrap_err(),
            DestinationError::NotPermitted
        );
        assert_eq!(
            resolve_checked(&d, false, &Fixed(vec![])).unwrap_err(),
            DestinationError::Unresolvable
        );
        // Names that mean this machine never reach DNS unless granted.
        for local in [
            "http://localhost/",
            "http://api.localhost/",
            "http://printer.local/",
        ] {
            assert_eq!(
                resolve_checked(&parse(local).unwrap(), false, &Fixed(vec!["93.184.216.34"]))
                    .unwrap_err(),
                DestinationError::NotPermitted,
                "{local}"
            );
        }
        // Explicitly granted private destinations resolve; `Never` stays never.
        let lo = parse("http://127.0.0.1:8080/").unwrap();
        assert!(resolve_checked(&lo, true, &Fixed(vec![])).is_ok());
        let multicast = parse("http://224.0.0.1/").unwrap();
        assert!(resolve_checked(&multicast, true, &Fixed(vec![])).is_err());
    }

    #[test]
    fn redirects_are_checked_like_any_url() {
        let d = parse("https://example.com/a/b").unwrap();
        assert_eq!(
            d.join("/c").unwrap().url().as_str(),
            "https://example.com/c"
        );
        assert_eq!(
            d.join("c?x=1").unwrap().url().as_str(),
            "https://example.com/a/c?x=1"
        );
        assert_eq!(
            d.join("file:///etc/passwd").unwrap_err(),
            DestinationError::Scheme
        );
        assert_eq!(
            d.join("//user@evil.example/").unwrap_err(),
            DestinationError::Userinfo
        );
        assert!(!d.join("https://other.example/").unwrap().same_origin(&d));
        assert_eq!(
            d.join("/x\r\nSet-Cookie: a").unwrap_err(),
            DestinationError::Unparsable
        );
    }
}
