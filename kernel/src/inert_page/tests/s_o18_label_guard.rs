//! S-O18 (a guard, not a proof): `POLICY_V1.label_passed` is the only
//! source of the pass text in the kernel, and it ends "(not rendered)"; the
//! S1a module makes no render or layout claim.

use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files(&path, out);
        } else if path
            .extension()
            .is_some_and(|x| x == "rs" || x == "css" || x == "html")
        {
            out.push(path);
        }
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn s_o18_the_pass_label_has_one_source_and_no_render_claim_appears() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let pass = ["inert page check: ", "passed"].concat();
    let mut all = Vec::new();
    files(&src, &mut all);
    let mut sources = Vec::new();
    for path in &all {
        let text = read(path);
        for (at, _) in text.match_indices(&pass) {
            let after: String = text[at + pass.len()..].chars().take(16).collect();
            sources.push((path.clone(), after));
        }
    }
    assert_eq!(sources.len(), 1, "one source of the pass text: {sources:?}");
    assert!(
        sources[0].0.ends_with("inert_page/policy.rs"),
        "{sources:?}"
    );
    assert!(sources[0].1.starts_with(" (not rendered)\""), "{sources:?}");

    let word = ["render", "ed"].concat();
    let claims = [
        ["verified", " layout"].concat(),
        ["layout", " verified"].concat(),
        ["page", " verified"].concat(),
        ["layout", " checked"].concat(),
    ];
    let mut module = Vec::new();
    files(&src.join("inert_page"), &mut module);
    assert!(module.len() > 5);
    for path in &module {
        let text = read(path).to_ascii_lowercase();
        for (at, _) in text.match_indices(&word) {
            assert!(
                text[..at].ends_with("not "),
                "{}: a claim at byte {at}",
                path.display()
            );
        }
        for claim in &claims {
            assert!(
                !text.contains(claim.as_str()),
                "{}: {claim:?}",
                path.display()
            );
        }
    }
}
