//! The typed page tree (S3's output) and its one canonical serialization
//! (S4's reference).

use super::policy::POLICY_V1;
use super::{check, Refusal, RefusalCode};

/// The 16 body tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tag {
    Header,
    Main,
    Footer,
    Section,
    H1,
    H2,
    H3,
    P,
    Address,
    Ul,
    Ol,
    Li,
    Strong,
    Em,
    Small,
    Br,
}

impl Tag {
    pub const fn name(self) -> &'static str {
        match self {
            Tag::Header => "header",
            Tag::Main => "main",
            Tag::Footer => "footer",
            Tag::Section => "section",
            Tag::H1 => "h1",
            Tag::H2 => "h2",
            Tag::H3 => "h3",
            Tag::P => "p",
            Tag::Address => "address",
            Tag::Ul => "ul",
            Tag::Ol => "ol",
            Tag::Li => "li",
            Tag::Strong => "strong",
            Tag::Em => "em",
            Tag::Small => "small",
            Tag::Br => "br",
        }
    }
}

/// The class tokens of Mother's stylesheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Lead,
    Muted,
    Center,
    Note,
    Plain,
}

impl Class {
    pub const fn name(self) -> &'static str {
        match self {
            Class::Lead => "lead",
            Class::Muted => "muted",
            Class::Center => "center",
            Class::Note => "note",
            Class::Plain => "plain",
        }
    }
}

/// An element's attributes: at most one class token and one id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attrs {
    pub class: Option<Class>,
    pub id: Option<String>,
}

/// Inline content. Text is held decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Br,
    Strong(Leaf),
    Em(Leaf),
    Small(Leaf),
}

/// A leaf block, list item or span: attributes and inline content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leaf {
    pub attrs: Attrs,
    pub inlines: Vec<Inline>,
}

/// A `ul` or `ol` and its `li` items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    pub attrs: Attrs,
    pub items: Vec<Leaf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    H1(Leaf),
    H2(Leaf),
    H3(Leaf),
    P(Leaf),
    Address(Leaf),
    Ul(List),
    Ol(List),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub attrs: Attrs,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MainItem {
    Section(Section),
    Block(Block),
}

/// A `header` or `footer`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub attrs: Attrs,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Main {
    pub attrs: Attrs,
    pub items: Vec<MainItem>,
}

/// An accepted page. It exists only through [`super::check`] or
/// [`Page::new`], so it always satisfies every rule of `POLICY_V1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub(super) lang: String,
    pub(super) title: String,
    pub(super) header: Option<Region>,
    pub(super) main: Main,
    pub(super) footer: Option<Region>,
}

impl Page {
    /// Builds a page from its parts under exactly the checks of
    /// [`super::check`]. A refusal points into the canonical serialization.
    pub fn new(
        lang: &str,
        title: &str,
        header: Option<Region>,
        main: Main,
        footer: Option<Region>,
    ) -> Result<Page, Refusal> {
        let page = Page {
            lang: lang.to_owned(),
            title: title.to_owned(),
            header,
            main,
            footer,
        };
        // Past the size cap the bytes are cut short: they are refused as
        // `too_large` either way.
        let bytes = write_page(&page, POLICY_V1.max_page_bytes);
        let parsed = check(&bytes)?;
        if parsed != page {
            // The bytes are canonical, but the tree is not the one they
            // parse to (adjacent or empty text nodes).
            return Err(Refusal::at(RefusalCode::NotCanonical, &bytes, 0));
        }
        Ok(page)
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// The decoded title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn header(&self) -> Option<&Region> {
        self.header.as_ref()
    }

    pub fn main(&self) -> &Main {
        &self.main
    }

    pub fn footer(&self) -> Option<&Region> {
        self.footer.as_ref()
    }
}

/// The canonical bytes of `page`, stopping once more than `limit` bytes are
/// written. It does not recurse, so a deep tree cannot exhaust the stack.
pub(super) fn write_page(page: &Page, limit: usize) -> Vec<u8> {
    let mut w = Writer {
        out: Vec::new(),
        limit,
    };
    let [head, after_lang, after_title, after_css] = POLICY_V1.head_segments;
    w.raw(head);
    w.raw(&page.lang);
    w.raw(after_lang);
    w.text(&page.title);
    w.raw(after_title);
    w.raw(POLICY_V1.css);
    w.raw(after_css);
    if let Some(header) = &page.header {
        w.container(Tag::Header, &header.attrs, |w| w.blocks(&header.blocks));
    }
    w.container(Tag::Main, &page.main.attrs, |w| {
        for item in &page.main.items {
            match item {
                MainItem::Section(s) => {
                    w.container(Tag::Section, &s.attrs, |w| w.blocks(&s.blocks))
                }
                MainItem::Block(b) => w.block(b),
            }
        }
    });
    if let Some(footer) = &page.footer {
        w.container(Tag::Footer, &footer.attrs, |w| w.blocks(&footer.blocks));
    }
    w.raw(POLICY_V1.epilogue);
    w.out
}

struct Writer {
    out: Vec<u8>,
    limit: usize,
}

impl Writer {
    fn raw(&mut self, s: &str) {
        if self.out.len() <= self.limit {
            self.out.extend_from_slice(s.as_bytes());
        }
    }

    /// Text, with each of the five special characters as its entity.
    fn text(&mut self, s: &str) {
        let mut buf = [0u8; 4];
        for c in s.chars() {
            if self.out.len() > self.limit {
                return;
            }
            match POLICY_V1.entities.iter().find(|(e, _)| *e == c) {
                Some((_, entity)) => self.raw(entity),
                None => self.raw(c.encode_utf8(&mut buf)),
            }
        }
    }

    fn open(&mut self, tag: Tag, attrs: &Attrs) {
        let [class_name, id_name] = POLICY_V1.body_attributes;
        self.raw("<");
        self.raw(tag.name());
        for (name, value) in [
            (class_name, attrs.class.map(Class::name)),
            (id_name, attrs.id.as_deref()),
        ] {
            if let Some(value) = value {
                self.raw(" ");
                self.raw(name);
                self.raw("=\"");
                self.raw(value);
                self.raw("\"");
            }
        }
        self.raw(">");
    }

    fn close(&mut self, tag: Tag) {
        self.raw("</");
        self.raw(tag.name());
        self.raw(">");
    }

    fn container(&mut self, tag: Tag, attrs: &Attrs, children: impl FnOnce(&mut Writer)) {
        self.open(tag, attrs);
        self.raw("\n");
        children(self);
        self.close(tag);
        self.raw("\n");
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for b in blocks {
            self.block(b);
        }
    }

    fn block(&mut self, b: &Block) {
        match b {
            Block::H1(l) => self.leaf(Tag::H1, l),
            Block::H2(l) => self.leaf(Tag::H2, l),
            Block::H3(l) => self.leaf(Tag::H3, l),
            Block::P(l) => self.leaf(Tag::P, l),
            Block::Address(l) => self.leaf(Tag::Address, l),
            Block::Ul(list) => self.list(Tag::Ul, list),
            Block::Ol(list) => self.list(Tag::Ol, list),
        }
    }

    fn list(&mut self, tag: Tag, list: &List) {
        self.container(tag, &list.attrs, |w| {
            for li in &list.items {
                w.leaf(Tag::Li, li);
            }
        });
    }

    fn leaf(&mut self, tag: Tag, leaf: &Leaf) {
        self.open(tag, &leaf.attrs);
        self.inlines(&leaf.inlines);
        self.close(tag);
        self.raw("\n");
    }

    /// Inline content, with an explicit stack of open spans.
    fn inlines(&mut self, inlines: &[Inline]) {
        let mut stack: Vec<(std::slice::Iter<'_, Inline>, Option<Tag>)> =
            vec![(inlines.iter(), None)];
        while let Some(top) = stack.last_mut() {
            let Some(item) = top.0.next() else {
                if let Some((_, Some(tag))) = stack.pop() {
                    self.close(tag);
                }
                continue;
            };
            let (tag, span) = match item {
                Inline::Text(t) => {
                    self.text(t);
                    continue;
                }
                Inline::Br => {
                    self.open(Tag::Br, &Attrs::default());
                    continue;
                }
                Inline::Strong(span) => (Tag::Strong, span),
                Inline::Em(span) => (Tag::Em, span),
                Inline::Small(span) => (Tag::Small, span),
            };
            self.open(tag, &span.attrs);
            stack.push((span.inlines.iter(), Some(tag)));
        }
    }
}
