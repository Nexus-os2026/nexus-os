//! S-O4: round trip over 2,000 seeded trees covering every tag, class,
//! entity character, a multibyte character, `br`, depth 8 and a space-only
//! text node between two spans: `check(serialize(t)) == Ok(t)`, and the
//! bytes are equal again after re-serialization.

use super::*;

/// Text characters: every entity character and multibyte characters; no
/// `:`, `(` or `@`, so no needle can form.
const ALPHABET: &[char] = &[
    'a', 'b', 'c', 'x', 'y', 'z', 'A', 'Q', 'Z', '0', '1', '9', '.', ',', '!', '?', '-', ';', '=',
    '*', '#', '%', '+', '_', '~', '|', '$', '&', '<', '>', '"', '\'', 'é', 'ü', 'ß', '€', '–', '—',
    '‘', '’', '“', '”', '•', '…', '™', '¡', '¿', 'ç', 'Ā', 'ſ', 'ÿ',
];
const ENTITY_CHARS: [char; 5] = ['&', '<', '>', '"', '\''];
const LANGS: [&str; 6] = ["en", "fr", "de", "en-GB", "pt-BR", "es"];
const CLASSES: [Class; 5] = [
    Class::Lead,
    Class::Muted,
    Class::Center,
    Class::Note,
    Class::Plain,
];
const TAGS: [Tag; 16] = [
    Tag::Header,
    Tag::Main,
    Tag::Footer,
    Tag::Section,
    Tag::H1,
    Tag::H2,
    Tag::H3,
    Tag::P,
    Tag::Address,
    Tag::Ul,
    Tag::Ol,
    Tag::Li,
    Tag::Strong,
    Tag::Em,
    Tag::Small,
    Tag::Br,
];
const BUDGET: usize = 250;

/// A leaf's flattened text before it is cut into spans: word chunks, and a
/// single space or `br` between words.
enum Atom {
    Chunk(String),
    Space,
    Br,
}

pub(super) struct Gen {
    rng: Rng,
    ids: usize,
    elements: usize,
}

impl Gen {
    pub(super) fn new(seed: u64) -> Gen {
        Gen {
            rng: Rng::new(seed),
            ids: 0,
            elements: 0,
        }
    }

    fn attrs(&mut self) -> Attrs {
        let class = self.rng.chance(30).then(|| *self.rng.pick(&CLASSES));
        let id = self.rng.chance(20).then(|| {
            self.ids += 1;
            if self.rng.chance(20) {
                format!("x{:031}", self.ids)
            } else {
                format!("i{}", self.ids)
            }
        });
        Attrs { class, id }
    }

    fn word(&mut self) -> String {
        (0..1 + self.rng.below(10))
            .map(|_| *self.rng.pick(ALPHABET))
            .collect()
    }

    fn atoms(&mut self, depth: usize) -> Vec<Atom> {
        let words = if self.elements < BUDGET {
            1 + self.rng.below(5)
        } else {
            1
        };
        let mut atoms = Vec::new();
        for i in 0..words {
            if i > 0 {
                let br = depth < 8 && self.rng.chance(20);
                atoms.push(if br { Atom::Br } else { Atom::Space });
            }
            let word: Vec<char> = self.word().chars().collect();
            let cut = 1 + self.rng.below(word.len());
            atoms.push(Atom::Chunk(word[..cut].iter().collect()));
            if cut < word.len() {
                atoms.push(Atom::Chunk(word[cut..].iter().collect()));
            }
        }
        atoms
    }

    fn push(&mut self, out: &mut Vec<Inline>, atom: &Atom) {
        let text = match atom {
            Atom::Br => {
                self.elements += 1;
                out.push(Inline::Br);
                return;
            }
            Atom::Chunk(s) => s.as_str(),
            Atom::Space => " ",
        };
        match out.last_mut() {
            Some(Inline::Text(t)) => t.push_str(text),
            _ => out.push(Inline::Text(text.to_string())),
        }
    }

    fn span(&mut self, inlines: Vec<Inline>, attrs: Attrs) -> Inline {
        self.elements += 1;
        let leaf = Leaf { attrs, inlines };
        match self.rng.below(3) {
            0 => Inline::Strong(leaf),
            1 => Inline::Em(leaf),
            _ => Inline::Small(leaf),
        }
    }

    /// `atoms` as the content of an element at depth `parent`, with random
    /// spans; a span at depth 8 holds no `br` (its depth would be 9).
    fn inlines(&mut self, atoms: &[Atom], parent: usize) -> Vec<Inline> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < atoms.len() {
            if parent < 8 && self.elements < BUDGET && self.rng.chance(25) {
                let mut j = i + 1 + self.rng.below(atoms.len() - i);
                if parent + 1 == 8 {
                    if let Some(k) = atoms[i..j].iter().position(|a| matches!(a, Atom::Br)) {
                        j = i + k;
                    }
                }
                if j > i {
                    let attrs = self.attrs();
                    let inner = self.inlines(&atoms[i..j], parent + 1);
                    let span = self.span(inner, attrs);
                    out.push(span);
                    i = j;
                    continue;
                }
            }
            self.push(&mut out, &atoms[i]);
            i += 1;
        }
        out
    }

    /// Every word in its own span, so the spaces between them are
    /// space-only text nodes between two spans.
    fn word_spans(&mut self, atoms: &[Atom]) -> Vec<Inline> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < atoms.len() {
            let end = atoms[i..]
                .iter()
                .position(|a| !matches!(a, Atom::Chunk(_)))
                .map_or(atoms.len(), |k| i + k);
            if end > i {
                let mut inner = Vec::new();
                for atom in &atoms[i..end] {
                    self.push(&mut inner, atom);
                }
                let attrs = self.attrs();
                let span = self.span(inner, attrs);
                out.push(span);
                i = end;
            } else {
                self.push(&mut out, &atoms[i]);
                i += 1;
            }
        }
        out
    }

    /// The first word nested in spans down to depth 8.
    fn deep(&mut self, atoms: &[Atom], depth: usize) -> Vec<Inline> {
        let first = atoms
            .iter()
            .position(|a| !matches!(a, Atom::Chunk(_)))
            .unwrap_or(atoms.len());
        let mut inner = Vec::new();
        for atom in &atoms[..first] {
            self.push(&mut inner, atom);
        }
        for _ in depth + 1..=8 {
            let attrs = self.attrs();
            inner = vec![self.span(inner, attrs)];
        }
        for atom in &atoms[first..] {
            self.push(&mut inner, atom);
        }
        inner
    }

    fn leaf(&mut self, depth: usize) -> Leaf {
        self.elements += 1;
        let attrs = self.attrs();
        let atoms = self.atoms(depth);
        let inlines = match self.rng.below(10) {
            _ if self.elements >= BUDGET => self.inlines(&atoms, 8),
            0 | 1 => self.word_spans(&atoms),
            2 => self.deep(&atoms, depth),
            _ => self.inlines(&atoms, depth),
        };
        Leaf { attrs, inlines }
    }

    fn count(&mut self, most: usize) -> usize {
        if self.elements < BUDGET {
            1 + self.rng.below(most)
        } else {
            1
        }
    }

    fn block(&mut self, depth: usize) -> Block {
        match self.rng.below(7) {
            0 => Block::H1(self.leaf(depth)),
            1 => Block::H2(self.leaf(depth)),
            2 => Block::H3(self.leaf(depth)),
            3 => Block::P(self.leaf(depth)),
            4 => Block::Address(self.leaf(depth)),
            k => {
                self.elements += 1;
                let attrs = self.attrs();
                let n = self.count(3);
                let items = (0..n).map(|_| self.leaf(depth + 1)).collect();
                let list = List { attrs, items };
                if k == 5 {
                    Block::Ul(list)
                } else {
                    Block::Ol(list)
                }
            }
        }
    }

    fn blocks(&mut self, depth: usize) -> Vec<Block> {
        let n = self.count(3);
        (0..n).map(|_| self.block(depth)).collect()
    }

    fn region(&mut self) -> Region {
        self.elements += 1;
        let attrs = self.attrs();
        Region {
            attrs,
            blocks: self.blocks(2),
        }
    }

    fn main(&mut self) -> Main {
        self.elements += 1;
        let attrs = self.attrs();
        let n = self.count(4);
        let items = (0..n)
            .map(|_| {
                if self.rng.chance(50) {
                    self.elements += 1;
                    let attrs = self.attrs();
                    MainItem::Section(Section {
                        attrs,
                        blocks: self.blocks(3),
                    })
                } else {
                    MainItem::Block(self.block(2))
                }
            })
            .collect();
        Main { attrs, items }
    }

    pub(super) fn page(&mut self) -> Page {
        self.elements = 0;
        let lang = *self.rng.pick(&LANGS);
        let words: Vec<String> = (0..1 + self.rng.below(5)).map(|_| self.word()).collect();
        let header = self.rng.chance(60).then(|| self.region());
        let main = self.main();
        let footer = self.rng.chance(60).then(|| self.region());
        Page::new(lang, &words.join(" "), header, main, footer)
            .unwrap_or_else(|r| panic!("a generated tree is refused: {r}"))
    }
}

/// `n` accepted pages from `seed`.
pub(super) fn generated_pages(seed: u64, n: usize) -> Vec<Vec<u8>> {
    let mut gen = Gen::new(seed);
    (0..n).map(|_| serialize(&gen.page())).collect()
}

#[derive(Default)]
struct Coverage {
    tags: Vec<Tag>,
    classes: Vec<Class>,
    entity_chars: Vec<char>,
    multibyte: bool,
    deepest: usize,
    space_between_spans: bool,
}

fn is_span(item: &Inline) -> bool {
    matches!(item, Inline::Strong(_) | Inline::Em(_) | Inline::Small(_))
}

impl Coverage {
    fn page(&mut self, p: &Page) {
        self.text(p.title());
        if let Some(h) = p.header() {
            self.element(Tag::Header, &h.attrs, 1);
            self.blocks(&h.blocks, 2);
        }
        self.element(Tag::Main, &p.main().attrs, 1);
        for item in &p.main().items {
            match item {
                MainItem::Section(s) => {
                    self.element(Tag::Section, &s.attrs, 2);
                    self.blocks(&s.blocks, 3);
                }
                MainItem::Block(b) => self.block(b, 2),
            }
        }
        if let Some(f) = p.footer() {
            self.element(Tag::Footer, &f.attrs, 1);
            self.blocks(&f.blocks, 2);
        }
    }

    fn element(&mut self, tag: Tag, attrs: &Attrs, depth: usize) {
        self.tags.push(tag);
        self.classes.extend(attrs.class);
        self.deepest = self.deepest.max(depth);
    }

    fn text(&mut self, text: &str) {
        for c in text.chars() {
            if ENTITY_CHARS.contains(&c) {
                self.entity_chars.push(c);
            }
            self.multibyte |= c.len_utf8() > 1;
        }
    }

    fn blocks(&mut self, blocks: &[Block], depth: usize) {
        for b in blocks {
            self.block(b, depth);
        }
    }

    fn block(&mut self, b: &Block, depth: usize) {
        match b {
            Block::H1(l) => self.leaf(Tag::H1, l, depth),
            Block::H2(l) => self.leaf(Tag::H2, l, depth),
            Block::H3(l) => self.leaf(Tag::H3, l, depth),
            Block::P(l) => self.leaf(Tag::P, l, depth),
            Block::Address(l) => self.leaf(Tag::Address, l, depth),
            Block::Ul(list) | Block::Ol(list) => {
                let tag = if matches!(b, Block::Ul(_)) {
                    Tag::Ul
                } else {
                    Tag::Ol
                };
                self.element(tag, &list.attrs, depth);
                for li in &list.items {
                    self.leaf(Tag::Li, li, depth + 1);
                }
            }
        }
    }

    fn leaf(&mut self, tag: Tag, leaf: &Leaf, depth: usize) {
        self.element(tag, &leaf.attrs, depth);
        let items = &leaf.inlines;
        for (k, item) in items.iter().enumerate() {
            match item {
                Inline::Text(t) => {
                    self.text(t);
                    if t == " "
                        && k > 0
                        && is_span(&items[k - 1])
                        && items.get(k + 1).is_some_and(is_span)
                    {
                        self.space_between_spans = true;
                    }
                }
                Inline::Br => self.element(Tag::Br, &Attrs::default(), depth + 1),
                Inline::Strong(l) => self.leaf(Tag::Strong, l, depth + 1),
                Inline::Em(l) => self.leaf(Tag::Em, l, depth + 1),
                Inline::Small(l) => self.leaf(Tag::Small, l, depth + 1),
            }
        }
    }

    fn assert_complete(&self) {
        for tag in TAGS {
            assert!(self.tags.contains(&tag), "no {}", tag.name());
        }
        for class in CLASSES {
            assert!(self.classes.contains(&class), "no class {}", class.name());
        }
        for c in ENTITY_CHARS {
            assert!(self.entity_chars.contains(&c), "no {c:?}");
        }
        assert!(self.multibyte, "no multibyte character");
        assert_eq!(self.deepest, 8, "depth 8 is reached and never passed");
        assert!(
            self.space_between_spans,
            "no space-only text node between two spans"
        );
    }
}

#[test]
fn s_o4_round_trip_of_2000_seeded_trees() {
    let mut gen = Gen::new(0x5EED_0004);
    let mut coverage = Coverage::default();
    for _ in 0..2_000 {
        let page = gen.page();
        coverage.page(&page);
        let bytes = serialize(&page);
        let parsed = check(&bytes).unwrap_or_else(|r| panic!("refused: {r}"));
        assert_eq!(parsed, page);
        assert_eq!(serialize(&parsed), bytes);
    }
    coverage.assert_complete();
}
