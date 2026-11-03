//! `nexus-mother`: Mother's desktop window (M2b-A).

fn main() -> std::process::ExitCode {
    let code = nexus_mother_desktop::app::main_entry();
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
