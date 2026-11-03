//! M3-O6 (design M3 §5.3): the Mother reader's independently authored
//! expected-text fixtures (`mother/src/oracles/m3a_reader_fixtures.rs`,
//! included unchanged) checked against the existing HTML5 oracle. html5ever
//! (0.29.1, through `scraper`; this crate is only the host, as for S-O7,
//! and no Cargo file changes) parses each fixture page with zero parse
//! errors in no-quirks mode, and its title text and each leaf element's
//! text (its text descendants joined, `br` as one space), tag for tag, are
//! exactly the hand-written texts. The kernel accepts each page, and its
//! own `Leaf::text` agrees too.
//!
//! html5ever is a proxy for a browser, not a browser. Nothing is laid out.

use nexus_kernel::inert_page::*;
use scraper::{ElementRef, Html, Node};

#[path = "../../../mother/src/oracles/m3a_reader_fixtures.rs"]
mod reader_fixtures;

const REFERENCE: &str = include_str!("../../../kernel/src/inert_page/fixtures/reference.html");
/// The leaf elements of S1a's grammar.
const LEAVES: [&str; 6] = ["h1", "h2", "h3", "p", "li", "address"];

/// An element's text: its text descendants joined, `br` as one space.
fn flat(e: ElementRef<'_>) -> String {
    e.children()
        .map(|c| match (ElementRef::wrap(c), c.value()) {
            (Some(child), _) if child.value().name() == "br" => " ".to_string(),
            (Some(child), _) => flat(child),
            (None, Node::Text(t)) => String::from(&**t),
            (None, _) => String::new(),
        })
        .collect()
}

/// Every element named `name` below `e`, in document order.
fn named<'a>(e: ElementRef<'a>, name: &str, out: &mut Vec<ElementRef<'a>>) {
    for child in e.children().filter_map(ElementRef::wrap) {
        if child.value().name() == name {
            out.push(child);
        }
        named(child, name, out);
    }
}

/// Each leaf element below `e` with its text, in document order.
fn leaf_lines(e: ElementRef<'_>, out: &mut Vec<(String, String)>) {
    for child in e.children().filter_map(ElementRef::wrap) {
        let name = child.value().name();
        if LEAVES.contains(&name) {
            out.push((name.to_string(), flat(child)));
        } else {
            leaf_lines(child, out);
        }
    }
}

/// The kernel's leaf lines: each leaf's tag and `Leaf::text`.
fn kernel_lines(page: &Page) -> Vec<(String, String)> {
    fn blocks(bs: &[Block], out: &mut Vec<(String, String)>) {
        for b in bs {
            let (tag, leaf) = match b {
                Block::H1(l) => ("h1", l),
                Block::H2(l) => ("h2", l),
                Block::H3(l) => ("h3", l),
                Block::P(l) => ("p", l),
                Block::Address(l) => ("address", l),
                Block::Ul(list) | Block::Ol(list) => {
                    for li in &list.items {
                        out.push(("li".to_string(), li.text()));
                    }
                    continue;
                }
            };
            out.push((tag.to_string(), leaf.text()));
        }
    }
    let mut out = Vec::new();
    if let Some(h) = page.header() {
        blocks(&h.blocks, &mut out);
    }
    for item in &page.main().items {
        match item {
            MainItem::Section(s) => blocks(&s.blocks, &mut out),
            MainItem::Block(b) => blocks(std::slice::from_ref(b), &mut out),
        }
    }
    if let Some(f) = page.footer() {
        blocks(&f.blocks, &mut out);
    }
    out
}

#[test]
fn m3_o6_the_hand_written_texts_are_html5ever_s_and_the_kernel_s() {
    assert_eq!(reader_fixtures::FIXTURES.len(), 8);
    for f in reader_fixtures::FIXTURES {
        let page = reader_fixtures::fixture_page(REFERENCE, f);
        let expected: Vec<(String, String)> = f
            .lines
            .iter()
            .map(|(t, s)| (t.to_string(), s.to_string()))
            .collect();

        let html = Html::parse_document(&page);
        assert!(html.errors.is_empty(), "{}: {:?}", f.label, html.errors);
        assert_eq!(format!("{:?}", html.quirks_mode), "NoQuirks", "{}", f.label);
        let root = html.root_element();
        let mut titles = Vec::new();
        named(root, "title", &mut titles);
        assert_eq!(titles.len(), 1, "{}", f.label);
        assert_eq!(
            flat(titles[0]),
            f.title_text,
            "{}: html5ever's title",
            f.label
        );
        let mut bodies = Vec::new();
        named(root, "body", &mut bodies);
        assert_eq!(bodies.len(), 1, "{}", f.label);
        let mut dom = Vec::new();
        leaf_lines(bodies[0], &mut dom);
        assert_eq!(dom, expected, "{}: html5ever", f.label);

        let parsed = check(page.as_bytes()).unwrap_or_else(|r| panic!("{}: {r}", f.label));
        assert_eq!(
            parsed.title(),
            f.title_text,
            "{}: the kernel's title",
            f.label
        );
        assert_eq!(kernel_lines(&parsed), expected, "{}: the kernel", f.label);
    }
}
