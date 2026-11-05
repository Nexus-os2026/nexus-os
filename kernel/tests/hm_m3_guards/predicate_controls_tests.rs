//! E-O15 controls for the R-2 literal and cfg oracle amendments.

use super::{planted, predicate_findings, source};

fn production_fixture(src: &str) -> source::ProdFile {
    let mut file = planted("kernel/src/guard_hardening_control.rs", src);
    let (ranges, _) = source::cfg_test_items(&file.code, &file.text);
    for view in [&mut file.code, &mut file.text] {
        let mut bytes = view.as_bytes().to_vec();
        for &(start, end) in &ranges {
            for byte in &mut bytes[start..end] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
        }
        *view = String::from_utf8(bytes).unwrap();
    }
    file
}

#[test]
fn e_o15_raw_c_literal_reviewer_bypass_is_red() {
    // Exact reviewer prefix, followed by both independently protected forms.
    let prefix = "pub const S: &std::ffi::CStr = cr#\"a\"b\"#;\n";
    for suffix in [
        "pub fn name_is_cloud() -> bool { false }",
        "pub const P: &str = \"cloud\";",
        "pub fn name_is_cloud() -> bool { false }\npub const P: &str = \"cloud\";",
    ] {
        let expected = predicate_findings(&[production_fixture(suffix)]).len();
        assert!(expected > 0);
        let src = format!("{prefix}{suffix}");
        assert_eq!(
            predicate_findings(&[production_fixture(&src)]).len(),
            expected,
            "raw C literal must not hide subsequent protected code: {src}"
        );
    }
}

#[test]
fn e_o15_all_literal_prefixes_preserve_protected_code_and_mask_contents() {
    for literal in [
        r#""ordinary \"quote\"""#,
        r##"r#"a"b"#"##,
        r###"br##"a"b"##"###,
        r#"b"ordinary \"quote\"""#,
        r#"c"ordinary \"quote\"""#,
        r##"cr#"a"b"#"##,
    ] {
        let src = format!("const S = {literal}; fn name_is_cloud() {{}}");
        let found = predicate_findings(&[production_fixture(&src)]);
        assert_eq!(
            found.len(),
            1,
            "must retain code after {literal}: {found:?}"
        );
    }
    for prefix in ["", "b", "c", "r#", "br#", "cr#"] {
        let tail = if prefix.ends_with('#') { "#" } else { "" };
        let src = format!("const S = {prefix}\"name_is_cloud\"{tail};");
        assert!(predicate_findings(&[production_fixture(&src)]).is_empty());
        let src = format!("const S = {prefix}\"cloud\"{tail};");
        assert_eq!(predicate_findings(&[production_fixture(&src)]).len(), 1);
    }
}

#[test]
fn e_o15_cfg_reviewer_bypass_is_production_and_genuine_test_gates_are_green() {
    for item in [
        "pub fn name_is_cloud() -> bool { false }",
        "pub const P: &str = \"cloud\";",
        "mod hidden { pub fn name_is_cloud() -> bool { false } }",
        "impl Alternate { pub fn name_is_cloud() -> bool { false } }",
    ] {
        let src = format!("#[cfg(all(any(test, unix)))]\n{item}");
        assert_eq!(
            predicate_findings(&[production_fixture(&src)]).len(),
            1,
            "Linux production item must remain visible: {src}"
        );
        for gate in ["test", "all(test, unix)", "not(not(test))"] {
            let src = format!("#[cfg({gate})]\n{item}");
            assert!(
                predicate_findings(&[production_fixture(&src)]).is_empty(),
                "genuinely test-only item: {src}"
            );
        }
    }
}
