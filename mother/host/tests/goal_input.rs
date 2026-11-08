//! M3-E M-3a (rulings M3E-R1, R8; register docs/security/m3-e-oracles.md):
//! a typed goal and a goal file have one grammar. `validate_goal` (the typed
//! path's validator) gives exactly what a goal file's snapshot parses to,
//! for the same text, refusals included.
#![cfg(target_os = "linux")]
#![deny(unsafe_code)]

use nexus_mother_host::inputs::{read_input, validate_goal, InputKind, InputValue};

#[test]
fn m3e_m3a_typed_and_file_goals_share_one_grammar() {
    let dir = std::env::temp_dir().join(format!("m3e-m3a-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let e4096 = "é".repeat(4096);
    let e4097 = "é".repeat(4097);
    let bread = "🍞".repeat(4096);
    let cases: Vec<Vec<u8>> = vec![
        b"  Make me a small page.  Keep it simple!  ".to_vec(),
        b"approve".to_vec(),
        "Bäckerei 面包🍞".into(),
        e4096.into_bytes(),
        e4097.into_bytes(),
        bread.into_bytes(),
        Vec::new(),
        b"   ".to_vec(),
        b"a\tb".to_vec(),
        "a\u{202E}b".into(),
        b"a\rb".to_vec(),
        vec![b'a', 0xff],
    ];
    for (i, text) in cases.iter().enumerate() {
        for file in [text.clone(), [text.as_slice(), b"\n"].concat()] {
            if file.len() > 16_384 {
                continue; // over the file cap: a read refusal, not grammar
            }
            let path = dir.join(format!("goal-{i}-{}", file.len()));
            std::fs::write(&path, &file).unwrap();
            let from_file = read_input(InputKind::Goal, &path)
                .unwrap()
                .parse()
                .map(|v| match v {
                    InputValue::Goal(g) => g,
                    _ => unreachable!("a goal snapshot parses to a goal"),
                });
            assert_eq!(validate_goal(text), from_file, "case {i}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
