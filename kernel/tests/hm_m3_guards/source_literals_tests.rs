//! D2 oracle amendment. Old: "raw r/br strings only; unknown forms can mis-mask later code".
//! New: "classify every Rust string/character prefix or abort the scan loudly".

use super::mask;

fn assert_literal(literal: &str, body: &str) {
    let src =
        format!("const S: () = {literal};\nfn name_is_cloud() {{}}\nconst P: &str = \"cloud\";\n");
    let code = mask(&src, false);
    let text = mask(&src, true);
    assert_eq!(code.len(), src.len(), "byte offsets: {literal}");
    assert_eq!(text, src, "literal-preserving view: {literal}");
    assert_eq!(
        code.match_indices('\n')
            .map(|(at, _)| at)
            .collect::<Vec<_>>(),
        src.match_indices('\n')
            .map(|(at, _)| at)
            .collect::<Vec<_>>(),
        "line offsets: {literal}"
    );
    let start = src.find(body).expect("body in fixture");
    assert!(
        code[start..start + body.len()]
            .bytes()
            .all(|b| b == b' ' || b == b'\n'),
        "literal body must be masked: {literal}"
    );
    assert!(
        code.contains("fn name_is_cloud() {}"),
        "code after {literal}"
    );
    let cloud = src.rfind("cloud").unwrap();
    assert_eq!(&code[cloud..cloud + 5], "     ", "later literal: {literal}");
    assert_eq!(code.matches('"').count(), 4, "only delimiters: {literal}");
}

#[test]
fn raw_c_embedded_quote_reviewer_bypass_keeps_protected_code_visible() {
    // Exact D2 reviewer shape: this used to swallow both the function and P.
    let src = r##"pub const S: &std::ffi::CStr = cr#"a"b"#;
pub fn name_is_cloud() -> bool { false }
pub const P: &str = "cloud";
"##;
    let code = mask(src, false);
    assert!(code.contains("pub fn name_is_cloud() -> bool { false }"));
    let open = src.rfind("\"cloud\"").unwrap();
    assert_eq!(&code[open..open + 7], "\"     \"");
    assert_eq!(mask(src, true), src);
}

#[test]
fn all_rust_string_prefixes_mask_embedded_quotes_and_preserve_offsets() {
    for prefix in ["", "b", "c"] {
        let body = "a\\\"b\\\\c\nline";
        assert_literal(&format!("{prefix}\"{body}\""), body);
    }
    for prefix in ["r", "br", "cr"] {
        for hashes in [0, 1, 2, 8, 255] {
            let marks = "#".repeat(hashes);
            let body = if hashes == 0 {
                "a\\b\nline"
            } else {
                "a\"b\\c\nline"
            };
            assert_literal(&format!("{prefix}{marks}\"{body}\"{marks}"), body);
        }
    }
    assert_literal("cr##\"é\"雪\nλ\"##", "é\"雪\nλ");
}

#[test]
fn character_literals_and_lifetimes_remain_distinct() {
    let src = r##"fn borrow<'a>(v: &'a str) -> &'a str { v }
fn chars() { let _ = 'é'; let _ = '\u{10_ffff}'; let _ = '\''; let _ = b'\xff'; }
fn name_is_cloud() {}
"##;
    let code = mask(src, false);
    assert!(code.contains("fn borrow<'a>(v: &'a str) -> &'a str { v }"));
    assert!(code.contains("let _ = '  ';"));
    assert!(!code.contains("10_ffff"));
    assert!(!code.contains("xff"));
    assert!(code.contains("fn name_is_cloud() {}"));
    assert_eq!(mask(src, true), src);
    assert_eq!(code.len(), src.len());
}

#[test]
fn unclassifiable_or_unterminated_literals_fail_loudly_in_both_views() {
    for src in [
        "const S: () = z\"unknown\";",
        "const S: () = z#\"unknown\"#;",
        "const S: () = rb\"unknown\";",
        "const S: () = c'c';",
        "const S: () = \"unterminated",
        "const S: () = b\"unterminated\\",
        "const S: () = c\"unterminated",
        "const S: () = r#\"unterminated\";",
        "const S: () = br##\"unterminated\"#;",
        "const S: () = cr#\"unterminated\";",
        "const S: () = '\\u{1234};",
        "const S: () = b'\\xff;",
    ] {
        for literals in [false, true] {
            assert!(
                std::panic::catch_unwind(|| mask(src, literals)).is_err(),
                "unclassifiable literal must abort scan ({literals}): {src}"
            );
        }
    }
}

#[test]
fn ordinary_raw_strings_identifiers_comments_and_unicode_stay_green() {
    let src = r###"fn r#type() { let café = r##"/* not a comment */ \""##; }
// cr#"ignored
/* nested /* c"ignored */ still ignored */
fn next() {}
"###;
    let code = mask(src, false);
    assert!(code.contains("fn r#type() { let café = r##\""));
    assert!(!code.contains("not a comment"));
    assert!(!code.contains("ignored"));
    assert!(code.contains("fn next() {}"));
    assert_eq!(code.len(), src.len());
}
