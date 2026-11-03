//! The typed page tree (S3's output) and its one canonical serialization
//! (S4's reference).

use super::policy::POLICY_V1;
use super::{Refusal, RefusalCode};

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
        let _ = (lang, title, header, main, footer);
        Err(Refusal::at(RefusalCode::NotCanonical, &[], 0))
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
/// written.
pub(super) fn write_page(page: &Page, limit: usize) -> Vec<u8> {
    let _ = (page, limit, &POLICY_V1);
    Vec::new()
}
