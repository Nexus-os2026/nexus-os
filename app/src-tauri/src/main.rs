#[allow(unexpected_cfgs)]
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn main() {
    nexus_desktop_backend::runtime::run();
}

#[cfg(not(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
)))]
fn main() {
    println!("NexusOS desktop backend (tauri-runtime disabled in this build)");
}
