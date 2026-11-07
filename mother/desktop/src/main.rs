//! `nexus-mother`: Mother's desktop window (M2b-A), and the Owner's
//! read-only `check-page` (M3-E M-2), dispatched before any home or GTK.

fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1).peekable();
    if args.peek().is_some_and(|first| first == "check-page") {
        args.next();
        #[cfg(target_os = "linux")]
        let code = nexus_mother_host::check_page::main_entry(args);
        #[cfg(not(target_os = "linux"))]
        let code = {
            println!("ERROR unsupported_platform");
            2
        };
        return std::process::ExitCode::from(code);
    }
    let code = nexus_mother_desktop::app::main_entry();
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
