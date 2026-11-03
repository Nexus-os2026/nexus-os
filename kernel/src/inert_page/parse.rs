//! S3: the strict parse of a candidate that passed S1 and S2 into the typed
//! tree. It reports the first error in document order.
//!
//! It accepts a superset of the canonical form in exactly three places,
//! which only S4 refuses: `id` before `class`, a raw `"` or `'` in text, and
//! a missing line feed after a block tag.

use std::collections::BTreeSet;

use super::policy::POLICY_V1;
use super::tree::{
    Attrs, Block, Class, Inline, Leaf, List, Main, MainItem, Page, Region, Section, Tag,
};
use super::RefusalCode as C;
use super::{work, RefusalCode};

/// A refusal code and the byte offset where it applies.
pub(super) type Fail = (RefusalCode, usize);

pub(super) fn parse_document(text: &str) -> Result<Page, Fail> {
    Parser {
        s: text.as_bytes(),
        text,
        pos: 0,
        open: Vec::new(),
        elements: 0,
        ids: BTreeSet::new(),
    }
    .document()
}

/// A tag name at the cursor: a body tag, or any other name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Name {
    Tag(Tag),
    Other,
}

/// What is at the cursor.
enum Next {
    Start(Name),
    End(Name),
    /// Text, or a `<` that opens no tag.
    Text,
    Eof,
}

/// The previous item of a leaf's flattened text (or the title's).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prev {
    Start,
    Char,
    Space,
    Br,
}

/// The spacing, word and length state of one flattened text.
struct Flow {
    prev: Prev,
    word: usize,
    chars: usize,
}

impl Flow {
    fn new() -> Flow {
        Flow {
            prev: Prev::Start,
            word: 0,
            chars: 0,
        }
    }

    fn char(&mut self, c: char, at: usize) -> Result<(), Fail> {
        self.chars += 1;
        if c == '\n' {
            return Err((C::TextSpacing, at));
        }
        if c == ' ' {
            if self.prev != Prev::Char {
                return Err((C::TextSpacing, at));
            }
            self.prev = Prev::Space;
            self.word = 0;
        } else {
            self.word += 1;
            if self.word > POLICY_V1.max_word_chars {
                return Err((C::WordTooLong, at));
            }
            self.prev = Prev::Char;
        }
        Ok(())
    }

    fn br(&mut self, at: usize) -> Result<(), Fail> {
        if self.prev != Prev::Char {
            return Err((C::TextSpacing, at));
        }
        self.prev = Prev::Br;
        self.word = 0;
        Ok(())
    }

    fn end(&self, at: usize) -> Result<(), Fail> {
        match self.prev {
            Prev::Space | Prev::Br => Err((C::TextSpacing, at)),
            Prev::Start | Prev::Char => Ok(()),
        }
    }
}

fn is_block(t: Tag) -> bool {
    matches!(
        t,
        Tag::H1 | Tag::H2 | Tag::H3 | Tag::P | Tag::Address | Tag::Ul | Tag::Ol
    )
}

fn lang_ok(v: &[u8]) -> bool {
    let lower = |b: &u8| b.is_ascii_lowercase();
    let upper = |b: &u8| b.is_ascii_uppercase();
    match v {
        [a, b] => lower(a) && lower(b),
        [a, b, b'-', c, d] => lower(a) && lower(b) && upper(c) && upper(d),
        _ => false,
    }
}

fn id_ok(v: &[u8]) -> bool {
    match v {
        [first, rest @ ..] => {
            first.is_ascii_lowercase()
                && v.len() <= POLICY_V1.max_id_chars
                && rest
                    .iter()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        }
        [] => false,
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

struct Parser<'a> {
    s: &'a [u8],
    text: &'a str,
    pos: usize,
    /// The open body elements, outermost first (its length is the depth).
    open: Vec<Tag>,
    elements: usize,
    ids: BTreeSet<&'a str>,
}

impl<'a> Parser<'a> {
    fn at(&self, lit: &str) -> bool {
        self.s[self.pos..].starts_with(lit.as_bytes())
    }

    /// Consumes `lit`, or fails with `code` at the first differing byte.
    fn fixed(&mut self, lit: &str, code: RefusalCode) -> Result<(), Fail> {
        work::add(lit.len());
        let same = lit
            .bytes()
            .zip(&self.s[self.pos..])
            .take_while(|(a, b)| a == *b)
            .count();
        if same < lit.len() {
            return Err((code, self.pos + same));
        }
        self.pos += lit.len();
        Ok(())
    }

    fn name_at(&self, from: usize) -> Name {
        let len = self.s[from..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric())
            .count();
        work::add(len + 1);
        let name = &self.s[from..from + len];
        POLICY_V1
            .body_tags
            .iter()
            .copied()
            .find(|t| t.name().as_bytes() == name)
            .map_or(Name::Other, Name::Tag)
    }

    fn peek(&self) -> Next {
        match &self.s[self.pos..] {
            [] => Next::Eof,
            [b'<', b'/', c, ..] if c.is_ascii_alphabetic() => Next::End(self.name_at(self.pos + 2)),
            [b'<', c, ..] if c.is_ascii_alphabetic() => Next::Start(self.name_at(self.pos + 1)),
            _ => Next::Text,
        }
    }

    fn peek_start(&self, tag: Tag) -> bool {
        matches!(self.peek(), Next::Start(Name::Tag(t)) if t == tag)
    }

    fn optional_lf(&mut self) {
        if self.s.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
        }
    }

    /// Where the input ends or holds something unexpected.
    fn misplaced(&self) -> Fail {
        match self.peek() {
            Next::Eof => (C::TagUnclosed, self.pos),
            _ => (C::ContentModel, self.pos),
        }
    }

    /// An end tag that does not close the current element.
    fn foreign_end(&self, name: Name, inline: bool) -> Fail {
        let code = match name {
            Name::Tag(Tag::Br) => C::VoidSyntax,
            Name::Tag(t) if !inline && self.open.contains(&t) => C::TagUnclosed,
            Name::Other if !inline => C::TagUnclosed,
            _ => C::TagMisnested,
        };
        (code, self.pos)
    }

    fn document(mut self) -> Result<Page, Fail> {
        let [head, after_lang, after_title, after_css] = POLICY_V1.head_segments;
        self.fixed(head, C::PrologueMismatch)?;
        let lang = self.lang()?;
        self.fixed(after_lang, C::PrologueMismatch)?;
        let title = self.title()?;
        self.fixed(after_title, C::PrologueMismatch)?;
        self.css()?;
        self.fixed(after_css, C::PrologueMismatch)?;
        let header = if self.peek_start(Tag::Header) {
            Some(self.region(Tag::Header)?)
        } else {
            None
        };
        if !self.peek_start(Tag::Main) {
            return Err(self.misplaced());
        }
        let main = self.main()?;
        let footer = if self.peek_start(Tag::Footer) {
            Some(self.region(Tag::Footer)?)
        } else {
            None
        };
        if !self.at(POLICY_V1.epilogue) {
            return Err(self.misplaced());
        }
        self.pos += POLICY_V1.epilogue.len();
        if self.pos != self.s.len() {
            return Err((C::TrailingContent, self.pos));
        }
        Ok(Page {
            lang,
            title,
            header,
            main,
            footer,
        })
    }

    fn lang(&mut self) -> Result<String, Fail> {
        let start = self.pos;
        let end = self.s[start..]
            .iter()
            .position(|&b| b == b'"')
            .map_or(self.s.len(), |k| start + k);
        work::add(end - start + 1);
        if !lang_ok(&self.s[start..end]) {
            return Err((C::LangInvalid, start));
        }
        self.pos = end;
        Ok(self.text[start..end].to_owned())
    }

    fn title(&mut self) -> Result<String, Fail> {
        let mut flow = Flow::new();
        let title = self.text_run(&mut flow, true)?;
        if matches!(self.peek(), Next::Start(_)) || flow.chars == 0 {
            return Err((C::TitleInvalid, self.pos));
        }
        flow.end(self.pos)?;
        Ok(title)
    }

    /// The stylesheet: every byte up to the first `</style` must be
    /// Mother's.
    fn css(&mut self) -> Result<(), Fail> {
        let css = POLICY_V1.css.as_bytes();
        let close = ["</", POLICY_V1.style_tag].concat();
        let rest = &self.s[self.pos..];
        let region = &rest[..find(rest, close.as_bytes()).unwrap_or(rest.len())];
        // Found by one window per byte, then compared byte for byte.
        work::add(2 * region.len() + 1);
        if region != css {
            let same = region.iter().zip(css).take_while(|(a, b)| a == b).count();
            return Err((C::CssMismatch, self.pos + same));
        }
        self.pos += css.len();
        Ok(())
    }

    /// Decoded text up to the next `<`, through `flow`. In the title, the
    /// length cap applies and a raw `>` is `title_invalid`.
    fn text_run(&mut self, flow: &mut Flow, title: bool) -> Result<String, Fail> {
        let mut out = String::new();
        while self.pos < self.s.len() && self.s[self.pos] != b'<' {
            work::add(1);
            let at = self.pos;
            let c = if self.s[at] == b'&' {
                let (c, entity) = POLICY_V1
                    .entities
                    .iter()
                    .find(|(_, e)| self.s[at..].starts_with(e.as_bytes()))
                    .ok_or((C::EntityNotAllowed, at))?;
                self.pos += entity.len();
                *c
            } else {
                let Some(c) = self.text[at..].chars().next() else {
                    break;
                };
                if c == '>' {
                    return Err((
                        if title {
                            C::TitleInvalid
                        } else {
                            C::ContentModel
                        },
                        at,
                    ));
                }
                self.pos += c.len_utf8();
                c
            };
            flow.char(c, at)?;
            if title && flow.chars > POLICY_V1.max_title_chars {
                return Err((C::TitleInvalid, at));
            }
            out.push(c);
        }
        Ok(out)
    }

    fn enter(&mut self, tag: Tag) -> Result<(), Fail> {
        work::add(1);
        self.elements += 1;
        if self.elements > POLICY_V1.max_elements {
            return Err((C::TooManyElements, self.pos));
        }
        if self.open.len() >= POLICY_V1.max_depth {
            return Err((C::DepthExceeded, self.pos));
        }
        self.open.push(tag);
        Ok(())
    }

    /// `<tag ATTRS>` at the cursor (a `br` is `<br>` exactly and is left at
    /// once).
    fn start_tag(&mut self, tag: Tag) -> Result<Attrs, Fail> {
        self.enter(tag)?;
        self.pos += 1 + tag.name().len();
        if tag == Tag::Br {
            if self.s.get(self.pos) != Some(&b'>') {
                return Err((C::VoidSyntax, self.pos));
            }
            self.pos += 1;
            self.open.pop();
            return Ok(Attrs::default());
        }
        self.attributes()
    }

    /// `</tag>` at the cursor; the element is left.
    fn end_tag(&mut self, tag: Tag) -> Result<(), Fail> {
        work::add(1);
        self.pos += 2 + tag.name().len();
        if self.s.get(self.pos) != Some(&b'>') {
            return Err((C::AttributeSyntax, self.pos));
        }
        self.pos += 1;
        self.open.pop();
        Ok(())
    }

    fn attributes(&mut self) -> Result<Attrs, Fail> {
        let [class_name, id_name] = POLICY_V1.body_attributes;
        let mut attrs = Attrs::default();
        let (mut has_class, mut has_id) = (false, false);
        loop {
            work::add(1);
            match self.s.get(self.pos) {
                Some(b'>') => {
                    self.pos += 1;
                    return Ok(attrs);
                }
                Some(b'/') => return Err((C::VoidSyntax, self.pos)),
                Some(b' ') => self.pos += 1,
                Some(_) => return Err((C::AttributeSyntax, self.pos)),
                None => return Err((C::TagUnclosed, self.pos)),
            }
            let name_at = self.pos;
            let len = self.s[name_at..]
                .iter()
                .take_while(|b| !matches!(b, b' ' | b'=' | b'>' | b'/' | b'\n' | b'"' | b'\''))
                .count();
            work::add(len);
            let name = &self.s[name_at..name_at + len];
            let is_class = name == class_name.as_bytes();
            let is_id = name == id_name.as_bytes();
            if name.is_empty() || (is_class && has_class) || (is_id && has_id) {
                return Err((C::AttributeSyntax, name_at));
            }
            if !is_class && !is_id {
                return Err((C::AttributeNotAllowed, name_at));
            }
            self.pos = name_at + len;
            if !self.at("=\"") {
                return Err((C::AttributeSyntax, self.pos));
            }
            let value_at = self.pos + 2;
            let Some(value_len) = self.s[value_at..].iter().position(|&b| b == b'"') else {
                work::add(self.s.len() - value_at);
                return Err((C::AttributeSyntax, value_at));
            };
            // Found, then checked (and, for an id, entered in the id set).
            work::add(3 * value_len + 1);
            let text: &'a str = self.text;
            let value = &text[value_at..value_at + value_len];
            if is_class {
                has_class = true;
                let class = POLICY_V1
                    .classes
                    .iter()
                    .copied()
                    .find(|c: &Class| c.name() == value)
                    .ok_or((C::ClassNotAllowed, value_at))?;
                attrs.class = Some(class);
            } else {
                has_id = true;
                if !id_ok(value.as_bytes()) {
                    return Err((C::IdInvalid, value_at));
                }
                if !self.ids.insert(value) {
                    return Err((C::IdDuplicate, value_at));
                }
                attrs.id = Some(value.to_owned());
            }
            self.pos = value_at + value_len + 1;
        }
    }

    /// The children of a container through its end tag.
    fn children(&mut self, container: Tag, sections: bool) -> Result<Vec<MainItem>, Fail> {
        let mut items = Vec::new();
        loop {
            work::add(1);
            match self.peek() {
                Next::Start(Name::Tag(Tag::Section)) if sections => {
                    items.push(MainItem::Section(self.section()?));
                }
                Next::Start(Name::Tag(t)) if is_block(t) => {
                    items.push(MainItem::Block(self.block(t)?))
                }
                Next::Start(_) | Next::Text => return Err((C::ContentModel, self.pos)),
                Next::End(Name::Tag(t)) if t == container => break,
                Next::End(name) => return Err(self.foreign_end(name, false)),
                Next::Eof => return Err((C::TagUnclosed, self.pos)),
            }
        }
        if items.is_empty() {
            return Err((C::EmptyElement, self.pos));
        }
        self.end_tag(container)?;
        self.optional_lf();
        Ok(items)
    }

    fn blocks(&mut self, container: Tag) -> Result<Vec<Block>, Fail> {
        let items = self.children(container, false)?;
        Ok(items
            .into_iter()
            .filter_map(|item| match item {
                MainItem::Block(b) => Some(b),
                MainItem::Section(_) => None,
            })
            .collect())
    }

    fn region(&mut self, tag: Tag) -> Result<Region, Fail> {
        let attrs = self.start_tag(tag)?;
        self.optional_lf();
        Ok(Region {
            attrs,
            blocks: self.blocks(tag)?,
        })
    }

    fn main(&mut self) -> Result<Main, Fail> {
        let attrs = self.start_tag(Tag::Main)?;
        self.optional_lf();
        Ok(Main {
            attrs,
            items: self.children(Tag::Main, true)?,
        })
    }

    fn section(&mut self) -> Result<Section, Fail> {
        let attrs = self.start_tag(Tag::Section)?;
        self.optional_lf();
        Ok(Section {
            attrs,
            blocks: self.blocks(Tag::Section)?,
        })
    }

    fn block(&mut self, t: Tag) -> Result<Block, Fail> {
        Ok(match t {
            Tag::H1 => Block::H1(self.leaf(t)?),
            Tag::H2 => Block::H2(self.leaf(t)?),
            Tag::H3 => Block::H3(self.leaf(t)?),
            Tag::P => Block::P(self.leaf(t)?),
            Tag::Address => Block::Address(self.leaf(t)?),
            Tag::Ul => Block::Ul(self.list(t)?),
            Tag::Ol => Block::Ol(self.list(t)?),
            _ => return Err((C::ContentModel, self.pos)),
        })
    }

    fn list(&mut self, t: Tag) -> Result<List, Fail> {
        let attrs = self.start_tag(t)?;
        self.optional_lf();
        let mut items = Vec::new();
        loop {
            work::add(1);
            match self.peek() {
                Next::Start(Name::Tag(Tag::Li)) => items.push(self.leaf(Tag::Li)?),
                Next::Start(_) | Next::Text => return Err((C::ContentModel, self.pos)),
                Next::End(Name::Tag(e)) if e == t => break,
                Next::End(name) => return Err(self.foreign_end(name, false)),
                Next::Eof => return Err((C::TagUnclosed, self.pos)),
            }
        }
        if items.is_empty() {
            return Err((C::EmptyElement, self.pos));
        }
        self.end_tag(t)?;
        self.optional_lf();
        Ok(List { attrs, items })
    }

    fn leaf(&mut self, t: Tag) -> Result<Leaf, Fail> {
        let attrs = self.start_tag(t)?;
        let mut flow = Flow::new();
        let inlines = self.inlines(t, &mut flow)?;
        flow.end(self.pos)?;
        self.end_tag(t)?;
        self.optional_lf();
        Ok(Leaf { attrs, inlines })
    }

    /// Inline content up to `</parent>`, which is left for the caller.
    fn inlines(&mut self, parent: Tag, flow: &mut Flow) -> Result<Vec<Inline>, Fail> {
        let mut out = Vec::new();
        loop {
            work::add(1);
            match self.peek() {
                Next::Start(Name::Tag(Tag::Br)) => {
                    flow.br(self.pos)?;
                    self.start_tag(Tag::Br)?;
                    out.push(Inline::Br);
                }
                Next::Start(Name::Tag(t @ (Tag::Strong | Tag::Em | Tag::Small))) => {
                    let attrs = self.start_tag(t)?;
                    let inlines = self.inlines(t, flow)?;
                    self.end_tag(t)?;
                    let span = Leaf { attrs, inlines };
                    out.push(match t {
                        Tag::Strong => Inline::Strong(span),
                        Tag::Em => Inline::Em(span),
                        _ => Inline::Small(span),
                    });
                }
                Next::Start(_) => return Err((C::ContentModel, self.pos)),
                Next::End(Name::Tag(t)) if t == parent => break,
                Next::End(name) => return Err(self.foreign_end(name, true)),
                Next::Text if self.s[self.pos] == b'<' => return Err((C::ContentModel, self.pos)),
                Next::Text => out.push(Inline::Text(self.text_run(flow, false)?)),
                Next::Eof => return Err((C::TagUnclosed, self.pos)),
            }
        }
        if out.is_empty() {
            return Err((C::EmptyElement, self.pos));
        }
        Ok(out)
    }
}
