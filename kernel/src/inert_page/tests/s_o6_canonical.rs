//! S-O6: S4 is load-bearing. Each of S3's three superset inputs passes S1,
//! S2 and S3 and is refused by S4 with `not_canonical`.

use super::super::parse::parse_document;
use super::*;

#[test]
fn s_o6_each_superset_input_passes_s3_and_fails_s4() {
    for (name, input) in [
        (
            "id before class",
            edit("<p class=\"lead\">", "<p id=\"intro\" class=\"lead\">"),
        ),
        ("raw apostrophe", edit("<h1>Rosie&#39;s", "<h1>Rosie's")),
        (
            "missing newline after </p>",
            edit("morning.</p>\n", "morning.</p>"),
        ),
    ] {
        assert!(
            lexical_findings(input.as_bytes()).is_empty(),
            "{name}: S1 and S2 pass"
        );
        assert!(parse_document(&input).is_ok(), "{name}: S3 accepts it");
        assert_eq!(
            code_of(input.as_bytes()),
            Some(RefusalCode::NotCanonical),
            "{name}"
        );
    }
}
