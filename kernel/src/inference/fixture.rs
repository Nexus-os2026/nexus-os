//! Test-only fixture authority (M3-Cq Cq-1a, gate G1).
//!
//! Every item here carries exactly `#[cfg(test)]` and at most `pub(crate)`
//! visibility, and the module itself is declared `#[cfg(test)]`, so fixture
//! code exists only in nexus-kernel's own unit-test harness. No library
//! compile, no downstream crate and no shipped binary contains it.
//!
//! A [`FixtureListener`] binds its own loopback listener and is the sole
//! owner of that listener's authority. Its typed [`FixtureEndpoint`] carries
//! the frozen fixture records and a weak reference to the authority; it is
//! never made from a string, an address or an existing socket.

#[cfg(test)]
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(test)]
use std::sync::{Arc, Weak};

#[cfg(test)]
use super::endpoint::LoopbackEndpoint;
#[cfg(test)]
use super::{Floor, Purpose};

/// One fixture qualification: a synthetic test constant, never measured
/// evidence.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FixtureRecord {
    pub(crate) purpose: Purpose,
    pub(crate) identity_sha256: [u8; 32],
    pub(crate) floor: Floor,
}

/// The listener-owning authority. `live` is true until its owner drops.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct FixtureAuthority {
    listener: TcpListener,
    live: AtomicBool,
}

#[cfg(test)]
impl FixtureAuthority {
    /// Whether the owner still holds the authority (SeqCst).
    #[allow(dead_code)] // first use: Cq-2a
    pub(crate) fn live(&self) -> bool {
        self.live.load(Ordering::SeqCst)
    }

    /// The exact bound listener's address, if it can be read.
    #[allow(dead_code)] // first use: Cq-2a
    pub(crate) fn local_addr(&self) -> Option<SocketAddr> {
        self.listener.local_addr().ok()
    }
}

/// The sole revocation owner of one fixture listener. Not `Clone`.
#[cfg(test)]
pub(crate) struct FixtureListener {
    authority: Arc<FixtureAuthority>,
    records: Arc<[FixtureRecord]>,
    addr: SocketAddr,
}

/// A typed fixture capability: the private fixture-source endpoint.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct FixtureEndpoint {
    endpoint: LoopbackEndpoint,
}

/// Whether two records share a (purpose, identity) key, whatever their
/// floors. P-Cq-22's mutation site.
#[cfg(test)]
fn duplicate_keys(records: &[FixtureRecord]) -> bool {
    records.iter().enumerate().any(|(index, record)| {
        records[index + 1..].iter().any(|other| {
            other.purpose == record.purpose && other.identity_sha256 == record.identity_sha256
        })
    })
}

#[cfg(test)]
impl FixtureListener {
    /// Bind `127.0.0.1:0` with frozen copies of `records`. Duplicate keys
    /// are `InvalidInput`. Empty records are permitted.
    #[allow(dead_code)] // first use: Cq-1b-k
    pub(crate) fn bind(records: &[FixtureRecord]) -> std::io::Result<Self> {
        if duplicate_keys(records) {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let addr = listener.local_addr()?;
        Ok(Self {
            authority: Arc::new(FixtureAuthority {
                listener,
                live: AtomicBool::new(true),
            }),
            records: Arc::from(records),
            addr,
        })
    }

    /// The typed endpoint for this listener.
    #[allow(dead_code)] // first use: Cq-1b-k
    pub(crate) fn endpoint(&self) -> FixtureEndpoint {
        FixtureEndpoint {
            endpoint: LoopbackEndpoint::fixture(
                self.addr,
                self.records.clone(),
                Arc::downgrade(&self.authority),
            ),
        }
    }

    /// A clone of the exact listening socket, for the trusted fake server.
    /// It carries no authority.
    #[allow(dead_code)] // first use: Cq-1b-k
    pub(crate) fn peer_listener(&self) -> std::io::Result<TcpListener> {
        self.authority.listener.try_clone()
    }

    /// The bound address (reporting only).
    #[allow(dead_code)] // first use: Cq-1b-k
    pub(crate) fn addr(&self) -> SocketAddr {
        self.addr
    }
}

#[cfg(test)]
impl Drop for FixtureListener {
    fn drop(&mut self) {
        self.authority.live.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
impl FixtureEndpoint {
    /// The address (reporting only; no conversion back).
    #[allow(dead_code)] // first use: Cq-1c
    pub(crate) fn addr(&self) -> SocketAddr {
        self.endpoint.addr()
    }

    /// The private endpoint, for `VerifiedOllama::verify_fixture` only.
    pub(super) fn into_endpoint(self) -> LoopbackEndpoint {
        self.endpoint
    }

    /// Whether the listener-owning authority is still strongly held
    /// (F-2's observation; no authority is exported).
    #[allow(dead_code)] // first use: Cq-1c
    pub(super) fn authority_alive_for_test(&self) -> bool {
        self.endpoint.fixture_authority_strong_count() > 0
    }
}

/// Weak authority helper used by the endpoint's private source.
#[cfg(test)]
pub(crate) type WeakAuthority = Weak<FixtureAuthority>;
