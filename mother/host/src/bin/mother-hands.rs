#![deny(unsafe_code)]
use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    let ok = nexus_mother_host::hands::run(std::env::args().skip(1));
    #[cfg(not(target_os = "linux"))]
    let ok = {
        eprintln!(
            "{}",
            nexus_mother_host::HandsStartupFailure::UnsupportedPlatform.code()
        );
        false
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
