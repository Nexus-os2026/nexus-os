//! Typed page construction and checked File request data, without effects.
use super::{
    draft::{DraftSection, PageDraft},
    BuildInputCode, DraftCode,
};
use crate::effect::EffectRequest;
use nexus_governed_control::{files::FileCreate, inert_page::*};

fn leaf(s: &str) -> Leaf {
    Leaf {
        attrs: Attrs::default(),
        inlines: vec![Inline::Text(s.into())],
    }
}
fn list<'a>(texts: impl Iterator<Item = &'a str>, class: Option<Class>) -> List {
    List {
        attrs: Attrs { class, id: None },
        items: texts.map(leaf).collect(),
    }
}
fn section(id: Option<&str>, blocks: Vec<Block>) -> MainItem {
    MainItem::Section(Section {
        attrs: Attrs {
            class: None,
            id: id.map(str::to_string),
        },
        blocks,
    })
}
fn build_page(d: &PageDraft, facts: &ApprovedFacts) -> Result<Page, DraftCode> {
    let mut lead = leaf(&d.lead);
    lead.attrs.class = Some(Class::Lead);
    let header = Region {
        attrs: Attrs::default(),
        blocks: vec![Block::H1(leaf(facts.name())), Block::P(lead)],
    };
    let mut items: Vec<_> = d
        .sections
        .iter()
        .map(|s| {
            let mut blocks = vec![Block::H2(leaf(&s.heading))];
            blocks.extend(s.paragraphs.iter().map(|p| Block::P(leaf(p))));
            if !s.items.is_empty() {
                blocks.push(Block::Ul(list(s.items.iter().map(String::as_str), None)));
            }
            section(None, blocks)
        })
        .collect();
    items.push(section(
        Some("hours"),
        vec![
            Block::H2(leaf("Opening hours")),
            Block::Ul(list(
                facts.hours().iter().map(String::as_str),
                Some(Class::Plain),
            )),
        ],
    ));
    let phone = Leaf {
        attrs: Attrs::default(),
        inlines: vec![
            Inline::Text("Phone: ".into()),
            Inline::Strong(leaf(facts.phone())),
        ],
    };
    items.push(section(
        Some("contact"),
        vec![Block::H2(leaf("Contact")), Block::P(phone)],
    ));
    let mut footer = leaf(&d.footer);
    footer.attrs.class = Some(Class::Muted);
    Page::new(
        "en",
        facts.name(),
        Some(header),
        Main {
            attrs: Attrs::default(),
            items,
        },
        Some(Region {
            attrs: Attrs::default(),
            blocks: vec![Block::P(footer)],
        }),
    )
    .map_err(|_| DraftCode::AssemblerFault)
}
pub(super) fn facts_ready(facts: &ApprovedFacts) -> Result<(), BuildInputCode> {
    let d = PageDraft {
        lead: "Welcome.".into(),
        sections: vec![DraftSection {
            heading: "About us".into(),
            paragraphs: vec!["Visit us.".into()],
            items: vec![],
        }],
        footer: "Thank you.".into(),
    };
    let p = build_page(&d, facts).map_err(|_| BuildInputCode::FactsNotPageReady)?;
    if check_facts(&p, facts).is_err() || crate::verifier::email_scan(&p).is_some() {
        return Err(BuildInputCode::FactsNotPageReady);
    }
    Ok(())
}
pub(super) fn assemble(d: &PageDraft, facts: &ApprovedFacts) -> Result<Vec<u8>, DraftCode> {
    let page = build_page(d, facts)?;
    let bytes = serialize(&page);
    let checked = check(&bytes).map_err(|_| DraftCode::AssemblerFault)?;
    if let Err(code) = check_facts(&checked, facts) {
        return Err(if code == FactCode::ForeignNumber {
            DraftCode::ForeignNumber
        } else {
            DraftCode::AssemblerFault
        });
    }
    if crate::verifier::email_scan(&checked).is_some() {
        return Err(DraftCode::Email);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| DraftCode::AssemblerFault)?;
    file_limits(text)?;
    Ok(bytes)
}
pub(super) fn file_limits(text: &str) -> Result<(), DraftCode> {
    if text.len() > 8192 || text.lines().count() > 150 {
        return Err(DraftCode::TooLarge);
    }
    Ok(())
}
fn create_file(content: String) -> Vec<EffectRequest> {
    #[cfg(test)]
    CONSTRUCTIONS.with(|c| c.set(c.get() + 1));
    vec![EffectRequest::CreateFile(FileCreate {
        name: "index.html".into(),
        content,
    })]
}
pub(super) fn file_requests(
    d: &PageDraft,
    facts: &ApprovedFacts,
) -> Result<Vec<EffectRequest>, DraftCode> {
    let bytes = assemble(d, facts)?;
    let content = String::from_utf8(bytes).map_err(|_| DraftCode::AssemblerFault)?;
    Ok(create_file(content))
}
#[cfg(test)]
thread_local! { static CONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn constructions() -> usize {
    CONSTRUCTIONS.with(|c| c.replace(0))
}
