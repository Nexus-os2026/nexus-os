//! S-O3 (S1a-A part): boundary pairs, accepted at the limit and refused one
//! past it. (The hours-line and phone bounds are S1a-B's.)

use super::*;
use RefusalCode as C;

fn pair(name: &str, at_limit: String, past: String, code: C) {
    if let Err(r) = check(at_limit.as_bytes()) {
        panic!("{name}: the page at the limit is refused: {r:?}");
    }
    assert_eq!(
        code_of(past.as_bytes()),
        Some(code),
        "{name}: one past the limit"
    );
}

#[test]
fn s_o3_boundaries_accept_at_the_limit_and_refuse_one_past() {
    pair("bytes", padded_to(65_536), padded_to(65_537), C::TooLarge);

    let phone = "<strong>01632 960123</strong>";
    // The phone's `p` is at depth 3 (main, section, p); spans go deeper.
    let eight = "<strong><em><small><strong><em>01632 960123</em></strong></small></em></strong>";
    let nine = "<strong><em><small><strong><em><small>01632 960123</small></em></strong></small></em></strong>";
    pair(
        "depth",
        edit(phone, eight),
        edit(phone, nine),
        C::DepthExceeded,
    );

    let items = |total: usize| {
        let extra = "<li>x</li>\n".repeat(total - reference_body_elements());
        edit(
            "<li>Sunday: closed</li>\n",
            &format!("<li>Sunday: closed</li>\n{extra}"),
        )
    };
    pair("elements", items(400), items(401), C::TooManyElements);

    // Characters, not bytes: each `é` is two bytes.
    pair(
        "word",
        edit("Exampleton", &"é".repeat(40)),
        edit("Exampleton", &"é".repeat(41)),
        C::WordTooLong,
    );

    // Decoded characters: `&#39;` is one.
    let title = |n: usize| {
        let text = format!("&#39;{} {}", "a".repeat(n - 42), "b".repeat(40));
        edit(
            "<title>Rosie&#39;s Bakery</title>",
            &format!("<title>{text}</title>"),
        )
    };
    pair("title", title(80), title(81), C::TitleInvalid);

    let id = |n: usize| edit("id=\"hours\"", &format!("id=\"h{}\"", "a".repeat(n - 1)));
    pair("id", id(32), id(33), C::IdInvalid);
}
