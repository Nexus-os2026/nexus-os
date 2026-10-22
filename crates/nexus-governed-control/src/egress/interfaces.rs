//! This machine's side of the network, as the kernel reports it now: the
//! address of every interface and the network directly connected through
//! it (the address with its prefix), and a point-to-point peer. Read with
//! `getifaddrs`, the one unsafe call outside the launcher, confined here.

#![allow(unsafe_code)]

use super::destination::LocalNetworks;

/// The system's view now. An error means it could not be read: the caller
/// fails closed.
#[cfg(target_os = "linux")]
pub(crate) fn system() -> std::io::Result<LocalNetworks> {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    /// The address in `address`, if it is an IPv4 or IPv6 one.
    ///
    /// # Safety
    /// `address` is null or points to a socket address `getifaddrs` gave,
    /// at least as long as its family says.
    unsafe fn ip(address: *const libc::sockaddr) -> Option<IpAddr> {
        if address.is_null() {
            return None;
        }
        match i32::from((*address).sa_family) {
            libc::AF_INET => {
                let v4 = &*(address as *const libc::sockaddr_in);
                Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(v4.sin_addr.s_addr))))
            }
            libc::AF_INET6 => {
                let v6 = &*(address as *const libc::sockaddr_in6);
                Some(IpAddr::V6(Ipv6Addr::from(v6.sin6_addr.s6_addr)))
            }
            _ => None,
        }
    }

    let full = |address: IpAddr| if address.is_ipv4() { 32 } else { 128 };
    // A netmask's prefix: its leading ones (a mask that is not contiguous
    // counts as the shorter, wider network).
    let prefix = |mask: IpAddr| -> u8 {
        match mask {
            IpAddr::V4(mask) => u32::from(mask).leading_ones() as u8,
            IpAddr::V6(mask) => u128::from(mask).leading_ones() as u8,
        }
    };
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: `getifaddrs` writes the head of a list it allocates into
    // `list`; it is freed below, once, with `freeifaddrs`.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut networks = Vec::new();
    let mut at = list;
    while !at.is_null() {
        // SAFETY: `at` is a node of the list `getifaddrs` returned, which
        // is not freed until the walk ends; its address fields are null or
        // point to socket addresses of the family they name.
        let entry = unsafe { &*at };
        if let Some(address) = unsafe { ip(entry.ifa_addr) } {
            let mask = unsafe { ip(entry.ifa_netmask) };
            let length = mask
                .filter(|mask| mask.is_ipv4() == address.is_ipv4())
                .map_or(full(address), prefix);
            networks.push((address, length));
            if entry.ifa_flags & libc::IFF_POINTOPOINT as u32 != 0 {
                // The peer of a point-to-point link is directly connected.
                if let Some(peer) = unsafe { ip(entry.ifa_ifu) } {
                    networks.push((peer, full(peer)));
                }
            }
        }
        at = entry.ifa_next;
    }
    // SAFETY: `list` came from `getifaddrs` and is freed exactly once.
    unsafe { libc::freeifaddrs(list) };
    Ok(LocalNetworks(networks))
}

/// Elsewhere the boundary cannot be read: public-only egress refuses.
#[cfg(not(target_os = "linux"))]
pub(crate) fn system() -> std::io::Result<LocalNetworks> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the local network boundary is read on Linux only",
    ))
}
