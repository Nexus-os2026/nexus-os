//! M3-O6's reader fixtures (design M3 §5.3, pre-registered): small
//! canonical pages, and their visible text written by hand from S1a's
//! grammar text (S1a design v2 §4.3 and §5: the five entities decoded in
//! one pass, spans joined, each `br` read as one space), never produced by
//! either implementation. Mother's second reader must give exactly these
//! texts (`m3a_verifier.rs`), and so must html5ever through the S-O7 host
//! (`kernel/tests/m3a_reader_html5.rs`, which includes this
//! file unchanged) and S1a's own `Leaf::text`.
//!
//! The file is self-contained, with no imports, so both crates can
//! include it.

/// One fixture: a title and a body as written in the page, and their
/// visible text as authored by hand.
pub struct ReaderFixture {
    pub label: &'static str,
    /// The title as it appears between `<title>` and `</title>`.
    pub title: &'static str,
    /// The body's lines as written, each ending in a line feed.
    pub body: &'static str,
    /// The decoded title.
    pub title_text: &'static str,
    /// Each leaf line's tag and visible text, in order.
    pub lines: &'static [(&'static str, &'static str)],
}

/// `reference` (S1a's reference page) with `f`'s title and body.
pub fn fixture_page(reference: &str, f: &ReaderFixture) -> String {
    let title = reference.find("<title>").expect("a title");
    let after_title = reference.find("</title>\n").expect("a title end");
    let body = reference.find("<body>\n").expect("a body") + "<body>\n".len();
    format!(
        "{}<title>{}{}{}</body>\n</html>\n",
        &reference[..title],
        f.title,
        &reference[after_title..body],
        f.body
    )
}

pub const FIXTURES: &[ReaderFixture] = &[
    ReaderFixture {
        label: "an entity in the title",
        title: "Rosa&#39;s Bakery",
        body: "<main>\n<p>Bread.</p>\n</main>\n",
        title_text: "Rosa's Bakery",
        lines: &[("p", "Bread.")],
    },
    ReaderFixture {
        label: "each of the five entities, and an encoded entity",
        title: "Salt &amp; &quot;Rye&quot; &lt;3 &gt; 2 &#39;s",
        body: "<main>\n<p>&amp; &lt; &gt; &quot; &#39;</p>\n<p>&amp;lt; is not &lt;</p>\n\
               <p>&amp;amp;&amp;#39;</p>\n</main>\n",
        title_text: "Salt & \"Rye\" <3 > 2 's",
        lines: &[
            ("p", "& < > \" '"),
            ("p", "&lt; is not <"),
            ("p", "&amp;&#39;"),
        ],
    },
    ReaderFixture {
        label: "nested spans",
        title: "Spans",
        body: "<main>\n<p><strong><em>a</em></strong> b</p>\n\
               <p><em>Warm <strong>rye <small>loaf</small></strong></em> today</p>\n</main>\n",
        title_text: "Spans",
        lines: &[("p", "a b"), ("p", "Warm rye loaf today")],
    },
    ReaderFixture {
        label: "one space between two spans",
        title: "Between",
        body: "<main>\n<p><strong>Open</strong> <em>today</em></p>\n<ul>\n\
               <li><small>x</small> <strong>y</strong> <em>z</em></li>\n</ul>\n</main>\n",
        title_text: "Between",
        lines: &[("p", "Open today"), ("li", "x y z")],
    },
    ReaderFixture {
        label: "br in mid-line, and small",
        title: "Breaks",
        body: "<main>\n<address>12 Mill Lane<br>Exampleton</address>\n\
               <p>Rye<br><strong>and</strong><br>spelt</p>\n\
               <p class=\"muted\"><small>Hours may change.</small></p>\n</main>\n",
        title_text: "Breaks",
        lines: &[
            ("address", "12 Mill Lane Exampleton"),
            ("p", "Rye and spelt"),
            ("p", "Hours may change."),
        ],
    },
    ReaderFixture {
        label: "every leaf tag, with attributes on containers and leaves",
        title: "Rosa&#39;s Bakery",
        body: "<header class=\"center\" id=\"top\">\n<h1 id=\"name\">Rosa&#39;s Bakery</h1>\n\
               <p class=\"lead\">Fresh bread.</p>\n</header>\n<main id=\"main\">\n\
               <section class=\"note\" id=\"hours\">\n<h2>Opening hours</h2>\n\
               <ul class=\"plain\">\n<li>Closed on Mondays</li>\n</ul>\n</section>\n\
               <h3 class=\"center\">Menu</h3>\n<ol id=\"menu\">\n<li id=\"rye\">Rye</li>\n\
               </ol>\n</main>\n<footer class=\"muted\">\n<address>Mill Lane</address>\n\
               </footer>\n",
        title_text: "Rosa's Bakery",
        lines: &[
            ("h1", "Rosa's Bakery"),
            ("p", "Fresh bread."),
            ("h2", "Opening hours"),
            ("li", "Closed on Mondays"),
            ("h3", "Menu"),
            ("li", "Rye"),
            ("address", "Mill Lane"),
        ],
    },
    ReaderFixture {
        label: "multibyte text, and spans with attributes",
        title: "Crème – brûlée",
        body: "<main>\n<p>Crème brûlée – 3 € … ™</p>\n\
               <p><strong class=\"note\">Rye</strong> <small id=\"s1\">and</small> \
               <em class=\"lead\">spelt</em></p>\n</main>\n",
        title_text: "Crème – brûlée",
        lines: &[("p", "Crème brûlée – 3 € … ™"), ("p", "Rye and spelt")],
    },
    ReaderFixture {
        label: "entities beside and inside spans",
        title: "Tom &amp; Jerry",
        body: "<main>\n<p>Tom &amp; <strong>Jerry&#39;s</strong> &quot;bakes&quot;</p>\n</main>\n",
        title_text: "Tom & Jerry",
        lines: &[("p", "Tom & Jerry's \"bakes\"")],
    },
];
