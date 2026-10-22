//! `nexus-verifier-sandbox`: the trusted Phase Two verifier sandbox helper.
//!
//! It is started only by the Nexus backend, reads nothing from its
//! arguments or environment, and takes its whole launch from the backend's
//! private control socket. On any platform other than x86_64 Linux it exits
//! immediately: verification is unavailable there.

fn main() {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    nexus_verifier_sandbox::helper::main();
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    std::process::exit(69);
}
