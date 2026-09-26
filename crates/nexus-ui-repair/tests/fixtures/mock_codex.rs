//! Finite native Codex fixture, compiled only by the vision_judge test harness.
use std::{env, fs, path::Path};

fn main() {
    let executable = env::current_exe().unwrap();
    let mode = executable.file_stem().unwrap().to_str().unwrap();
    match mode {
        "mock_codex_fail" => {
            eprintln!("simulated codex failure");
            std::process::exit(7);
        }
        "mock_codex_no_output" => return,
        "mock_codex" | "mock_codex_garbage" => {}
        _ => panic!("unknown fixture mode"),
    }
    let args: Vec<_> = env::args_os().skip(1).collect();
    let Some(output) = args
        .windows(2)
        .find(|pair| pair[0] == "--output-last-message")
        .map(|pair| Path::new(&pair[1]))
    else {
        eprintln!("mock_codex: --output-last-message not provided");
        std::process::exit(2);
    };
    let content = if mode == "mock_codex_garbage" {
        "this is not json {{\n"
    } else {
        concat!(
            r#"{"verdict":"Changed","confidence":0.92,"reasoning":"button highlighted","detected_changes":["highlight"]}"#,
            "\n"
        )
    };
    fs::write(output, content).unwrap();
}
