//! S-O7 (design v2 §8): the HTML5 differential. html5ever (0.29.1, through
//! `scraper`; hosted as a kernel integration test) parses
//! the reference page, every S-O4 page and the inline example
//! `<p><strong>Open</strong> <em>today</em></p>`, and must build exactly the
//! kernel's typed tree:
//! - zero parse errors, and no-quirks mode;
//! - element names, namespaces and attributes equal;
//! - a text node is ignored only if it is `\n`-only and its parent is `html
//!   head body header main footer section ul ol` (the grammar's separators).
//!   Every other text node, the one space between two spans included, is
//!   compared exactly after decoding, and any other text in a container
//!   position fails;
//! - `br` is compared as an element in place, and each leaf's and span's
//!   flattened text (spans joined, `br` as U+0020) equals the kernel's
//!   `Leaf::text`, the text the facts check matches against.
//!
//! The comparator has a self-test: a tree and a DOM that differ only by one
//! space between two spans give a mismatch. And each dangerous S-O2 plant
//! really contains its construct when html5ever parses it.
//!
//! html5ever is a proxy for a browser, not a browser. Nothing is laid out.

use nexus_kernel::inert_page::*;
use scraper::{ElementRef, Html, Node};
use sha2::{Digest, Sha256};

/// S-O4's generator, included unchanged from the kernel's oracle, so this
/// test parses exactly S-O4's pages (their digest is pinned on both sides).
#[path = "../src/inert_page/tests/s_o4_round_trip.rs"]
mod s_o4_round_trip;

/// The kernel oracles' local generator (`kernel/src/inert_page/tests/mod.rs`),
/// restated for S-O4's included generator.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const REFERENCE: &str = include_str!("../src/inert_page/fixtures/reference.html");
const S_O4_SEED: u64 = 0x5EED_0004;
const S_O4_PAGES: usize = 2_000;
/// The same digest as `kernel/src/inert_page/tests/s_o7_corpus.rs`.
const S_O4_CORPUS_SHA256: &str = "3400966d8489c6b9176e03a201809b81c2321fd953e619bc38416e9672cd1ed5";
const XHTML: &str = "http://www.w3.org/1999/xhtml";
/// The parents whose `\n`-only text nodes are the grammar's separators.
const CONTAINERS: [&str; 9] = [
    "html", "head", "body", "header", "main", "footer", "section", "ul", "ol",
];
/// The elements that hold inline content: leaves and spans.
const TEXT_ELEMENTS: [&str; 9] = [
    "h1", "h2", "h3", "p", "address", "li", "strong", "em", "small",
];

/// `base` with its one occurrence of `from` replaced by `to`.
fn edit(base: &str, from: &str, to: &str) -> String {
    assert_eq!(base.matches(from).count(), 1, "anchor {from:?}");
    base.replacen(from, to, 1)
}

/// The reference page with the inline example in the contact section.
fn inline_example() -> String {
    edit(
        REFERENCE,
        "</address>\n",
        "</address>\n<p><strong>Open</strong> <em>today</em></p>\n",
    )
}

/// One node of a document, as both sides are compared.
#[derive(Clone, Debug, PartialEq, Eq)]
enum N {
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        name: String,
        ns: String,
        attrs: Vec<(String, String)>,
        kids: Vec<N>,
    },
    Text(String),
    Comment(String),
    Other(String),
}

fn element(name: &str, attrs: &[(&str, &str)], kids: Vec<N>) -> N {
    let mut attrs: Vec<(String, String)> = attrs
        .iter()
        .map(|(a, v)| (a.to_string(), v.to_string()))
        .collect();
    attrs.sort();
    N::Element {
        name: name.to_string(),
        ns: XHTML.to_string(),
        attrs,
        kids,
    }
}

// The kernel's typed tree, as the document a browser should build from it.

fn with_attrs(name: &str, a: &Attrs, kids: Vec<N>) -> N {
    let mut attrs = Vec::new();
    if let Some(class) = a.class {
        attrs.push(("class", class.name()));
    }
    if let Some(id) = &a.id {
        attrs.push(("id", id.as_str()));
    }
    element(name, &attrs, kids)
}

fn inline(item: &Inline) -> N {
    match item {
        Inline::Text(t) => N::Text(t.clone()),
        Inline::Br => element("br", &[], Vec::new()),
        Inline::Strong(s) => leaf("strong", s),
        Inline::Em(s) => leaf("em", s),
        Inline::Small(s) => leaf("small", s),
    }
}

fn leaf(name: &str, l: &Leaf) -> N {
    with_attrs(name, &l.attrs, l.inlines.iter().map(inline).collect())
}

fn block(b: &Block) -> N {
    match b {
        Block::H1(l) => leaf("h1", l),
        Block::H2(l) => leaf("h2", l),
        Block::H3(l) => leaf("h3", l),
        Block::P(l) => leaf("p", l),
        Block::Address(l) => leaf("address", l),
        Block::Ul(list) => with_attrs("ul", &list.attrs, list_items(list)),
        Block::Ol(list) => with_attrs("ol", &list.attrs, list_items(list)),
    }
}

fn list_items(list: &List) -> Vec<N> {
    list.items.iter().map(|li| leaf("li", li)).collect()
}

fn region(name: &str, r: &Region) -> N {
    with_attrs(name, &r.attrs, r.blocks.iter().map(block).collect())
}

fn expected_document(page: &Page) -> Vec<N> {
    let mut body = Vec::new();
    if let Some(h) = page.header() {
        body.push(region("header", h));
    }
    let main = page.main();
    let items = main
        .items
        .iter()
        .map(|item| match item {
            MainItem::Section(s) => {
                with_attrs("section", &s.attrs, s.blocks.iter().map(block).collect())
            }
            MainItem::Block(b) => block(b),
        })
        .collect();
    body.push(with_attrs("main", &main.attrs, items));
    if let Some(f) = page.footer() {
        body.push(region("footer", f));
    }
    let viewport = [
        ("name", "viewport"),
        ("content", "width=device-width, initial-scale=1"),
    ];
    vec![
        N::Doctype {
            name: "html".to_string(),
            public_id: String::new(),
            system_id: String::new(),
        },
        element(
            "html",
            &[("lang", page.lang())],
            vec![
                element(
                    "head",
                    &[],
                    vec![
                        element("meta", &[("charset", "utf-8")], Vec::new()),
                        element("meta", &viewport, Vec::new()),
                        element("title", &[], vec![N::Text(page.title().to_string())]),
                        element("style", &[], vec![N::Text(format!("\n{MOTHER_CSS}"))]),
                    ],
                ),
                element("body", &[], body),
            ],
        ),
    ]
}

/// Every leaf's and span's `Leaf::text`, in document order.
fn kernel_texts(page: &Page) -> Vec<String> {
    fn push(l: &Leaf, out: &mut Vec<String>) {
        out.push(l.text());
        for item in &l.inlines {
            if let Inline::Strong(s) | Inline::Em(s) | Inline::Small(s) = item {
                push(s, out);
            }
        }
    }
    fn blocks(bs: &[Block], out: &mut Vec<String>) {
        for b in bs {
            match b {
                Block::H1(l) | Block::H2(l) | Block::H3(l) | Block::P(l) | Block::Address(l) => {
                    push(l, out)
                }
                Block::Ul(list) | Block::Ol(list) => {
                    for li in &list.items {
                        push(li, out);
                    }
                }
            }
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

// html5ever's document.

fn node_leaf(n: &Node) -> N {
    match n {
        Node::Text(t) => N::Text(String::from(&**t)),
        Node::Comment(c) => N::Comment(String::from(&**c)),
        other => N::Other(format!("{other:?}")),
    }
}

fn dom_element(e: ElementRef<'_>) -> N {
    let v = e.value();
    let mut attrs: Vec<(String, String)> = v
        .attrs()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    attrs.sort();
    let kids = e
        .children()
        .map(|c| match ElementRef::wrap(c) {
            Some(child) => dom_element(child),
            None => node_leaf(c.value()),
        })
        .collect();
    N::Element {
        name: v.name().to_string(),
        ns: v.name.ns.to_string(),
        attrs,
        kids,
    }
}

fn dom(html: &Html) -> Vec<N> {
    html.tree
        .root()
        .children()
        .map(|c| match c.value() {
            Node::Doctype(d) => N::Doctype {
                name: d.name().to_string(),
                public_id: d.public_id().to_string(),
                system_id: d.system_id().to_string(),
            },
            Node::Element(_) => dom_element(ElementRef::wrap(c).expect("an element")),
            other => node_leaf(other),
        })
        .collect()
}

/// Each leaf's and span's flattened text in html5ever's document: its text
/// descendants joined, `br` as U+0020, in document order.
fn dom_texts(nodes: &[N], out: &mut Vec<String>) {
    fn flat(nodes: &[N]) -> String {
        nodes
            .iter()
            .map(|n| match n {
                N::Text(t) => t.clone(),
                N::Element { name, .. } if name == "br" => " ".to_string(),
                N::Element { kids, .. } => flat(kids),
                _ => String::new(),
            })
            .collect()
    }
    for n in nodes {
        if let N::Element { name, kids, .. } = n {
            if TEXT_ELEMENTS.contains(&name.as_str()) {
                out.push(flat(kids));
            }
            dom_texts(kids, out);
        }
    }
}

// The comparator.

/// The grammar's separators: `\n`-only text in a container position.
fn ignorable(node: &N, parent: &str) -> bool {
    matches!(node, N::Text(t) if !t.is_empty() && t.bytes().all(|b| b == b'\n'))
        && CONTAINERS.contains(&parent)
}

/// The first difference between the expected and the actual children of
/// `parent`, if any.
fn mismatch(expected: &[N], actual: &[N], parent: &str) -> Option<String> {
    let e: Vec<&N> = expected.iter().filter(|n| !ignorable(n, parent)).collect();
    let a: Vec<&N> = actual.iter().filter(|n| !ignorable(n, parent)).collect();
    if e.len() != a.len() {
        return Some(format!(
            "in {parent}: {} children expected, {} found: {a:?}",
            e.len(),
            a.len()
        ));
    }
    for (x, y) in e.iter().zip(&a) {
        match (x, y) {
            (
                N::Element {
                    name,
                    ns,
                    attrs,
                    kids,
                },
                N::Element {
                    name: name2,
                    ns: ns2,
                    attrs: attrs2,
                    kids: kids2,
                },
            ) => {
                if name != name2 || ns != ns2 || attrs != attrs2 {
                    return Some(format!(
                        "in {parent}: expected <{name} {attrs:?}> ({ns}), found <{name2} {attrs2:?}> ({ns2})"
                    ));
                }
                if let Some(m) = mismatch(kids, kids2, name) {
                    return Some(m);
                }
            }
            _ if x != y => return Some(format!("in {parent}: expected {x:?}, found {y:?}")),
            _ => {}
        }
    }
    None
}

/// The whole differential for `page` and the bytes html5ever parses.
fn differential(page: &Page, bytes: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let html = Html::parse_document(text);
    if !html.errors.is_empty() {
        return Err(format!("parse errors: {:?}", html.errors));
    }
    if html.quirks_mode != Html::new_document().quirks_mode
        || format!("{:?}", html.quirks_mode) != "NoQuirks"
    {
        return Err(format!("quirks mode {:?}", html.quirks_mode));
    }
    let actual = dom(&html);
    if let Some(m) = mismatch(&expected_document(page), &actual, "#document") {
        return Err(m);
    }
    let mut d = Vec::new();
    dom_texts(&actual, &mut d);
    let k = kernel_texts(page);
    if d != k {
        let at = d.iter().zip(&k).take_while(|(a, b)| a == b).count();
        return Err(format!(
            "element text {at} of {}/{}: html5ever {:?}, kernel {:?}",
            d.len(),
            k.len(),
            d.get(at),
            k.get(at)
        ));
    }
    Ok(())
}

#[test]
fn s_o7_the_reference_the_inline_example_and_every_s_o4_page_parse_as_their_tree() {
    for (label, bytes) in [
        ("the reference page", REFERENCE.to_string()),
        ("the inline example", inline_example()),
    ] {
        let page = check(bytes.as_bytes()).unwrap_or_else(|r| panic!("{label}: {r}"));
        assert_eq!(serialize(&page), bytes.as_bytes(), "{label}");
        if let Err(why) = differential(&page, bytes.as_bytes()) {
            panic!("{label}: {why}");
        }
    }

    let mut gen = s_o4_round_trip::Gen::new(S_O4_SEED);
    let trees: Vec<Page> = (0..S_O4_PAGES).map(|_| gen.page()).collect();
    let pages: Vec<Vec<u8>> = trees.iter().map(serialize).collect();
    assert_eq!(
        pages,
        s_o4_round_trip::generated_pages(S_O4_SEED, S_O4_PAGES)
    );
    let mut h = Sha256::new();
    for page in &pages {
        h.update((page.len() as u64).to_be_bytes());
        h.update(page);
    }
    assert_eq!(
        hex::encode(h.finalize()),
        S_O4_CORPUS_SHA256,
        "S-O4's corpus"
    );
    let mut failures = Vec::new();
    for (k, (tree, bytes)) in trees.iter().zip(&pages).enumerate() {
        if let Err(why) = differential(tree, bytes) {
            failures.push(format!("S-O4 page {k}: {why}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {S_O4_PAGES} pages differ; the first: {}",
        failures.len(),
        failures
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn s_o7_comparator_self_test() {
    let good = inline_example();
    let page = check(good.as_bytes()).expect("the inline example is accepted");
    assert_eq!(differential(&page, good.as_bytes()), Ok(()), "control");
    let expected = expected_document(&page);
    let parsed = |text: &str| dom(&Html::parse_document(text));

    // One space between two spans dropped: a mismatch, in the tree and in
    // the element texts.
    let without = parsed(&edit(&good, "</strong> <em>", "</strong><em>"));
    assert!(
        mismatch(&expected, &without, "#document").is_some(),
        "one inter-inline space must be a mismatch"
    );
    let mut d = Vec::new();
    dom_texts(&without, &mut d);
    assert_ne!(d, kernel_texts(&page));

    // A `\n` inside a leaf is not a separator.
    let in_leaf = parsed(&edit(&good, "<em>today</em></p>", "<em>today</em>\n</p>"));
    assert!(
        mismatch(&expected, &in_leaf, "#document").is_some(),
        "a newline inside a leaf"
    );

    // Other text in a container position fails.
    let stray = parsed(&edit(
        &good,
        "<em>today</em></p>\n",
        "<em>today</em></p>\nx\n",
    ));
    assert!(
        mismatch(&expected, &stray, "#document").is_some(),
        "text in a container position"
    );

    // `\n`-only text in a container position is a separator, however long.
    let blank = parsed(&edit(
        &good,
        "<em>today</em></p>\n",
        "<em>today</em></p>\n\n",
    ));
    assert_eq!(mismatch(&expected, &blank, "#document"), None);
}

fn any(nodes: &[N], found: &dyn Fn(&N) -> bool) -> bool {
    nodes.iter().any(|n| {
        found(n)
            || match n {
                N::Element { kids, .. } => any(kids, found),
                _ => false,
            }
    })
}

fn count(nodes: &[N], found: &dyn Fn(&N) -> bool) -> usize {
    nodes
        .iter()
        .map(|n| {
            usize::from(found(n))
                + match n {
                    N::Element { kids, .. } => count(kids, found),
                    _ => 0,
                }
        })
        .sum()
}

fn is_element(n: &N, tag: &str) -> bool {
    matches!(n, N::Element { name, .. } if name == tag)
}

fn has_attribute(n: &N, tag: &str, attribute: &str, value: Option<&str>) -> bool {
    matches!(n, N::Element { name, attrs, .. } if name == tag
        && attrs.iter().any(|(a, v)| a == attribute && value.is_none_or(|x| x == v)))
}

#[test]
fn s_o7_each_dangerous_s_o2_plant_really_contains_its_construct() {
    use RefusalCode as C;
    type Construct = Box<dyn Fn(&[N]) -> bool>;
    let element_named = |tag: &'static str| -> Construct {
        Box::new(move |d: &[N]| any(d, &|n| is_element(n, tag)))
    };
    let attribute = |tag: &'static str, a: &'static str, v: Option<&'static str>| -> Construct {
        Box::new(move |d: &[N]| any(d, &|n| has_attribute(n, tag, a, v)))
    };
    let comment = |text: &'static str| -> Construct {
        Box::new(move |d: &[N]| any(d, &|n| matches!(n, N::Comment(c) if c == text)))
    };
    let text = |part: &'static str| -> Construct {
        Box::new(move |d: &[N]| any(d, &|n| matches!(n, N::Text(t) if t.contains(part))))
    };
    let phone = "<strong>01632 960123</strong>";
    let in_main = |t: &str| edit(REFERENCE, "<main>\n", &format!("<main>\n{t}"));
    let refresh = "<meta http-equiv=\"refresh\" content=\"0\">\n";

    let mut plants: Vec<(C, String, String, Construct)> = vec![
        (
            C::ScriptElement,
            "<script>".into(),
            edit(REFERENCE, "<p>Phone: ", "<p>Phone: <script>x</script>"),
            element_named("script"),
        ),
        (
            C::ScriptElement,
            "<SCRIPT>".into(),
            edit(REFERENCE, "<p>Phone: ", "<p>Phone: <SCRIPT>x</SCRIPT>"),
            element_named("script"),
        ),
        (
            C::StyleElement,
            "<style> in main".into(),
            in_main("<style>p{}</style>\n"),
            Box::new(|d: &[N]| count(d, &|n| is_element(n, "style")) == 2),
        ),
        (
            C::ElementNotAllowed,
            "<base href> in head".into(),
            edit(REFERENCE, "<head>\n", "<head>\n<base href=\"x\">\n"),
            attribute("base", "href", None),
        ),
        (
            C::EventHandler,
            "onclick=".into(),
            edit(REFERENCE, "<p>Phone: ", "<p onclick=\"x\">Phone: "),
            attribute("p", "onclick", None),
        ),
        (
            C::EventHandler,
            "ONLOAD=".into(),
            edit(REFERENCE, "<p>Phone: ", "<p ONLOAD=\"x\">Phone: "),
            attribute("p", "onload", None),
        ),
        (
            C::StyleAttribute,
            "style=".into(),
            edit(REFERENCE, "<p>Phone: ", "<p style=\"x\">Phone: "),
            attribute("p", "style", None),
        ),
        (
            C::UrlAttribute,
            "href=".into(),
            edit(REFERENCE, "<p>Phone: ", "<p href=\"x\">Phone: "),
            attribute("p", "href", None),
        ),
        (
            C::UrlAttribute,
            "src=".into(),
            edit(REFERENCE, "<p>Phone: ", "<p src=\"x\">Phone: "),
            attribute("p", "src", None),
        ),
        (
            C::AttributeNotAllowed,
            "hidden".into(),
            edit(REFERENCE, "<p>Phone: ", "<p hidden>Phone: "),
            attribute("p", "hidden", None),
        ),
        (
            C::AttributeNotAllowed,
            "meta refresh in head".into(),
            edit(
                REFERENCE,
                "<meta charset=\"utf-8\">\n",
                &format!("<meta charset=\"utf-8\">\n{refresh}"),
            ),
            attribute("meta", "http-equiv", Some("refresh")),
        ),
        (
            C::AttributeNotAllowed,
            "meta refresh in body".into(),
            in_main(refresh),
            attribute("meta", "http-equiv", Some("refresh")),
        ),
        (
            C::Comment,
            "comment".into(),
            in_main("<!-- x -->\n"),
            comment(" x "),
        ),
        (
            C::MarkupDeclaration,
            "CDATA".into(),
            edit(REFERENCE, "Exampleton", "<![CDATA[x]]>Exampleton"),
            comment("[CDATA[x]]"),
        ),
        (
            C::ProcessingInstruction,
            "<?".into(),
            edit(REFERENCE, "Exampleton", "<?x?>Exampleton"),
            comment("?x?"),
        ),
        (
            C::EntityNotAllowed,
            "&#106;avascript".into(),
            edit(REFERENCE, "Mill Lane", "&#106;avascript"),
            text("12 javascript"),
        ),
        (
            C::EntityNotAllowed,
            "&#x6A;".into(),
            edit(REFERENCE, "Mill Lane", "&#x6A;"),
            text("12 j"),
        ),
        (
            C::EntityNotAllowed,
            "&nbsp;".into(),
            edit(REFERENCE, "Mill Lane", "&nbsp;"),
            text("12 \u{A0}"),
        ),
        (
            C::EntityNotAllowed,
            "&AMP;".into(),
            edit(REFERENCE, "Mill Lane", "&AMP;"),
            text("12 &"),
        ),
    ];
    for name in [
        "a", "img", "svg", "math", "table", "iframe", "object", "embed", "link", "base",
        "noscript", "template", "form", "input", "button", "video", "audio", "canvas", "picture",
        "pre", "div", "span",
    ] {
        plants.push((
            C::ElementNotAllowed,
            format!("<{name}>"),
            edit(REFERENCE, phone, &format!("<{name}>01632 960123</{name}>")),
            element_named(name),
        ));
    }
    for (code, label, input, construct) in &plants {
        assert_eq!(
            check(input.as_bytes()).err().map(|r| r.code),
            Some(*code),
            "{label}: S-O2's refusal"
        );
        let d = dom(&Html::parse_document(input));
        assert!(construct(&d), "{label}: html5ever's document lacks it");
    }
    assert_eq!(plants.len(), 19 + 22);
}
