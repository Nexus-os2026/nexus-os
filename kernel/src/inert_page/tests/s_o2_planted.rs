//! S-O2: planted negatives, at least one per refusal code (design §8.1).
//! Each is a minimal edit of the reference page (a cap plant is padded or
//! nested to one past its limit) and is refused with exactly its code. Every
//! S1 and S2 plant also appears in `lexical_findings`; every S3 and S4 plant
//! has no lexical finding at all.

use super::*;
use RefusalCode as C;

const PHONE: &str = "<strong>01632 960123</strong>";
const CONTACT: &str = "<h2>Contact</h2>";
const TOWN: &str = "Exampleton";

/// The reference page with `text` inserted after the `<main>` line.
fn in_main(text: &str) -> String {
    edit("<main>\n", &format!("<main>\n{text}"))
}

struct Plant {
    code: C,
    label: &'static str,
    input: Vec<u8>,
    /// An S1 or S2 plant (not an S3 or S4 one).
    lexical: bool,
}

struct Corpus {
    plants: Vec<Plant>,
    lexical: bool,
}

impl Corpus {
    fn add(&mut self, code: C, label: &'static str, input: impl Into<Vec<u8>>) {
        let lexical = self.lexical;
        self.plants.push(Plant {
            code,
            label,
            input: input.into(),
            lexical,
        });
    }
}

fn plants() -> Vec<Plant> {
    let mut c = Corpus {
        plants: Vec::new(),
        lexical: true,
    };

    // S1
    let mut not_utf8 = REFERENCE.as_bytes().to_vec();
    not_utf8.insert(REFERENCE.find(TOWN).unwrap() + 3, 0xFF);
    c.add(C::NotUtf8, "byte FF", not_utf8);
    c.add(C::TooLarge, "65,537 bytes", padded_to(65_537));
    c.add(C::ByteOrderMark, "BOM", format!("\u{FEFF}{REFERENCE}"));
    c.add(C::CarriageReturn, "CRLF", REFERENCE.replace('\n', "\r\n"));
    c.add(
        C::NoFinalNewline,
        "no final newline",
        REFERENCE[..REFERENCE.len() - 1].to_string(),
    );
    c.add(C::NoFinalNewline, "empty", String::new());
    for (label, ch) in [
        ("NUL", '\0'),
        ("TAB", '\t'),
        ("U+0007", '\u{7}'),
        ("U+0085", '\u{85}'),
    ] {
        c.add(
            C::ControlChar,
            label,
            edit(TOWN, &format!("Example{ch}ton")),
        );
    }
    for (label, ch) in [
        ("U+202E", '\u{202E}'),
        ("U+2066", '\u{2066}'),
        ("U+200B", '\u{200B}'),
        ("U+FEFF", '\u{FEFF}'),
        ("U+00AD", '\u{AD}'),
        ("U+E0041", '\u{E0041}'),
        ("U+FE0F", '\u{FE0F}'),
    ] {
        c.add(
            C::InvisibleChar,
            label,
            edit(TOWN, &format!("Example{ch}ton")),
        );
    }
    for (label, ch) in [
        ("Cyrillic A", '\u{410}'),
        ("U+00A0", '\u{A0}'),
        ("U+3000", '\u{3000}'),
    ] {
        c.add(
            C::CharNotAllowed,
            label,
            edit(TOWN, &format!("Example{ch}ton")),
        );
    }

    // S2
    c.add(
        C::ScriptElement,
        "<script>",
        edit("<p>Phone: ", "<p>Phone: <script>x</script>"),
    );
    c.add(
        C::ScriptElement,
        "<SCRIPT>",
        edit("<p>Phone: ", "<p>Phone: <SCRIPT>x</SCRIPT>"),
    );
    c.add(
        C::StyleElement,
        "<style> in main",
        in_main("<style>p{}</style>\n"),
    );
    for name in [
        "a", "img", "svg", "math", "table", "iframe", "object", "embed", "link", "base",
        "noscript", "template", "form", "input", "button", "video", "audio", "canvas", "picture",
        "pre", "div", "span",
    ] {
        c.add(
            C::ElementNotAllowed,
            "a refused element",
            edit(PHONE, &format!("<{name}>01632 960123</{name}>")),
        );
    }
    c.add(
        C::ElementNotAllowed,
        "<base href> in head",
        edit("<head>\n", "<head>\n<base href=\"x\">\n"),
    );
    c.add(C::TagCase, "<P>", edit("<p>Phone: ", "<P>Phone: "));
    c.add(
        C::EventHandler,
        "onclick=",
        edit("<p>Phone: ", "<p onclick=\"x\">Phone: "),
    );
    c.add(
        C::EventHandler,
        "ONLOAD=",
        edit("<p>Phone: ", "<p ONLOAD=\"x\">Phone: "),
    );
    c.add(
        C::StyleAttribute,
        "style=",
        edit("<p>Phone: ", "<p style=\"x\">Phone: "),
    );
    c.add(
        C::UrlAttribute,
        "href=",
        edit("<p>Phone: ", "<p href=\"x\">Phone: "),
    );
    c.add(
        C::UrlAttribute,
        "src=",
        edit("<p>Phone: ", "<p src=\"x\">Phone: "),
    );
    c.add(
        C::AttributeNotAllowed,
        "title=",
        edit("<p>Phone: ", "<p title=\"x\">Phone: "),
    );
    c.add(
        C::AttributeNotAllowed,
        "hidden",
        edit("<p>Phone: ", "<p hidden>Phone: "),
    );
    let refresh = "<meta http-equiv=\"refresh\" content=\"0\">\n";
    c.add(
        C::AttributeNotAllowed,
        "meta refresh in head",
        edit(
            "<meta charset=\"utf-8\">\n",
            &format!("<meta charset=\"utf-8\">\n{refresh}"),
        ),
    );
    c.add(
        C::AttributeNotAllowed,
        "meta refresh in body",
        in_main(refresh),
    );
    c.add(C::Comment, "comment", in_main("<!-- x -->\n"));
    c.add(
        C::MarkupDeclaration,
        "CDATA",
        edit(TOWN, "<![CDATA[x]]>Exampleton"),
    );
    c.add(
        C::MarkupDeclaration,
        "second doctype",
        edit("<body>\n", "<body>\n<!doctype html>\n"),
    );
    c.add(
        C::ProcessingInstruction,
        "<?",
        edit(TOWN, "<?x?>Exampleton"),
    );
    for (label, text) in [
        ("&#106;avascript", "&#106;avascript"),
        ("&#x6A;", "&#x6A;"),
        ("&nbsp;", "&nbsp;"),
        ("bare &", "Mill & Lane"),
        ("&AMP;", "&AMP;"),
    ] {
        c.add(C::EntityNotAllowed, label, edit("Mill Lane", text));
    }
    for (code, text) in [
        (C::RemoteReference, "x://y"),
        (C::CssUrl, "url(x)"),
        (C::CssUrl, "URL(x)"),
        (C::CssImport, "@import"),
        (C::CssImport, "@IMPORT"),
        (C::JavascriptScheme, "javascript:x"),
        (C::JavascriptScheme, "JavaScript:x"),
        (C::DataScheme, "data:x"),
        (C::DataScheme, "DaTa:x"),
    ] {
        c.add(
            code,
            "needle",
            edit(CONTACT, &format!("<h2>Contact {text}</h2>")),
        );
    }

    // S3
    c.lexical = false;
    c.add(
        C::PrologueMismatch,
        "<!DOCTYPE html>",
        edit("<!doctype html>", "<!DOCTYPE html>"),
    );
    c.add(
        C::PrologueMismatch,
        "UTF-8",
        edit("charset=\"utf-8\"", "charset=\"UTF-8\""),
    );
    c.add(
        C::PrologueMismatch,
        "initial-scale=1.0",
        edit("initial-scale=1\"", "initial-scale=1.0\""),
    );
    c.add(
        C::PrologueMismatch,
        "no viewport",
        edit(
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
            "",
        ),
    );
    c.add(
        C::PrologueMismatch,
        "an extra meta",
        edit(
            "<meta charset=\"utf-8\">\n",
            "<meta charset=\"utf-8\">\n<meta name=\"robots\" content=\"none\">\n",
        ),
    );
    c.add(
        C::AttributeNotAllowed,
        "lang on a p",
        edit("<p>Phone: ", "<p lang=\"fr\">Phone: "),
    );
    c.add(C::LangInvalid, "EN", edit("lang=\"en-GB\"", "lang=\"EN\""));
    c.add(
        C::LangInvalid,
        "en-gb",
        edit("lang=\"en-GB\"", "lang=\"en-gb\""),
    );
    c.add(
        C::CssMismatch,
        "one byte changed",
        edit("#1d1b18", "#1d1b19"),
    );
    let last_rule = ".plain{list-style:none;padding-left:0}\n";
    c.add(C::CssMismatch, "truncated", edit(last_rule, ""));
    c.add(
        C::CssMismatch,
        "an extra rule",
        edit(last_rule, &format!("{last_rule}p{{position:fixed}}\n")),
    );
    let title = "<title>Rosie&#39;s Bakery</title>";
    c.add(
        C::TitleInvalid,
        "empty title",
        edit(title, "<title></title>"),
    );
    let long = format!("<title>{} {}</title>", "a".repeat(40), "b".repeat(40));
    c.add(C::TitleInvalid, "81 characters", edit(title, &long));
    c.add(
        C::TitleInvalid,
        "<em> in the title",
        edit(title, "<title><em>Rosie</em></title>"),
    );
    c.add(
        C::ContentModel,
        "section in header",
        edit("<header>\n", "<header>\n<section>\n<p>x</p>\n</section>\n"),
    );
    c.add(C::ContentModel, "li in main", in_main("<li>x</li>\n"));
    c.add(
        C::ContentModel,
        "p in p",
        edit(PHONE, "<p>x</p>01632 960123"),
    );
    c.add(
        C::ContentModel,
        "two main",
        edit("</main>\n", "</main>\n<main>\n<p>x</p>\n</main>\n"),
    );
    c.add(
        C::ContentModel,
        "no main",
        edit_in(&edit("<main>\n", ""), "</main>\n", ""),
    );
    let header_start = REFERENCE.find("<header>").unwrap();
    let header_end = REFERENCE.find("</header>\n").unwrap() + "</header>\n".len();
    let header = &REFERENCE[header_start..header_end];
    let moved = edit_in(
        &edit(header, ""),
        "</main>\n",
        &format!("</main>\n{header}"),
    );
    c.add(C::ContentModel, "header after main", moved);
    c.add(
        C::ContentModel,
        "ul in li",
        edit(
            "<li>Sunday: closed</li>",
            "<li>Sunday: <ul>\n<li>x</li>\n</ul>\n</li>",
        ),
    );
    c.add(
        C::TagUnclosed,
        "section unclosed",
        edit("</section>\n</main>", "</main>"),
    );
    c.add(
        C::TagMisnested,
        "strong/em",
        edit(PHONE, "<strong><em>01632 960123</strong></em>"),
    );
    c.add(C::VoidSyntax, "<br/>", edit("<br>", "<br/>"));
    c.add(C::VoidSyntax, "</br>", edit("<br>", "</br>"));
    c.add(C::VoidSyntax, "<br></br>", edit("<br>", "<br></br>"));
    let lead = "<p class=\"lead\">";
    c.add(C::AttributeSyntax, "unquoted", edit(lead, "<p class=lead>"));
    c.add(
        C::AttributeSyntax,
        "single quotes",
        edit(lead, "<p class='lead'>"),
    );
    c.add(
        C::AttributeSyntax,
        "two spaces",
        edit(lead, "<p  class=\"lead\">"),
    );
    c.add(
        C::AttributeSyntax,
        "duplicate",
        edit(lead, "<p class=\"lead\" class=\"lead\">"),
    );
    c.add(C::ClassNotAllowed, "hero", edit(lead, "<p class=\"hero\">"));
    c.add(
        C::ClassNotAllowed,
        "lead note",
        edit(lead, "<p class=\"lead note\">"),
    );
    c.add(C::IdInvalid, "Hours", edit("id=\"hours\"", "id=\"Hours\""));
    c.add(C::IdInvalid, "9x", edit("id=\"hours\"", "id=\"9x\""));
    c.add(
        C::IdInvalid,
        "33 characters",
        edit("id=\"hours\"", &format!("id=\"h{}\"", "a".repeat(32))),
    );
    c.add(
        C::IdDuplicate,
        "duplicate id",
        edit("id=\"contact\"", "id=\"hours\""),
    );
    c.add(C::EmptyElement, "<p></p>", in_main("<p></p>\n"));
    let ul_start = REFERENCE.find("<ul class=\"plain\">\n").unwrap();
    let ul_end = REFERENCE.find("</ul>").unwrap();
    c.add(
        C::EmptyElement,
        "empty ul",
        edit(&REFERENCE[ul_start..ul_end], "<ul class=\"plain\">\n"),
    );
    c.add(
        C::EmptyElement,
        "empty strong",
        edit(PHONE, "<strong></strong>"),
    );
    c.add(
        C::TextSpacing,
        "leading space",
        edit(CONTACT, "<h2> Contact</h2>"),
    );
    c.add(
        C::TextSpacing,
        "trailing space",
        edit(CONTACT, "<h2>Contact </h2>"),
    );
    c.add(
        C::TextSpacing,
        "double space",
        edit("Fresh bread", "Fresh  bread"),
    );
    c.add(
        C::TextSpacing,
        "space before br",
        edit("Lane<br>", "Lane <br>"),
    );
    c.add(C::TextSpacing, "two br", edit("<br>", "<br><br>"));
    c.add(
        C::TextSpacing,
        "br first",
        edit("<address>12", "<address><br>12"),
    );
    c.add(
        C::TextSpacing,
        "newline in text",
        edit(CONTACT, "<h2>Con\ntact</h2>"),
    );
    c.add(
        C::TextSpacing,
        "title leading space",
        edit("<title>Rosie", "<title> Rosie"),
    );
    c.add(
        C::TrailingContent,
        "trailing newline",
        format!("{REFERENCE}\n"),
    );
    c.add(
        C::DepthExceeded,
        "9 levels",
        edit(
            PHONE,
            "<strong><em><small><strong><em><small>01632 960123</small></em></strong></small></em></strong>",
        ),
    );
    let extra = "<li>x</li>\n".repeat(401 - reference_body_elements());
    c.add(
        C::TooManyElements,
        "401 elements",
        edit(
            "<li>Sunday: closed</li>\n",
            &format!("<li>Sunday: closed</li>\n{extra}"),
        ),
    );
    c.add(C::WordTooLong, "41 characters", edit(TOWN, &"a".repeat(41)));

    // S4
    c.add(
        C::NotCanonical,
        "id before class",
        edit(lead, "<p id=\"intro\" class=\"lead\">"),
    );
    c.add(
        C::NotCanonical,
        "raw apostrophe",
        edit("<h1>Rosie&#39;s", "<h1>Rosie's"),
    );
    c.add(
        C::NotCanonical,
        "missing newline after </p>",
        edit("morning.</p>\n", "morning.</p>"),
    );

    c.plants
}

#[test]
fn s_o2_every_plant_is_refused_with_exactly_its_code() {
    let plants = plants();
    let mut failures = Vec::new();
    for Plant {
        code,
        label,
        input,
        lexical,
    } in &plants
    {
        let got = check(input).err();
        if got.map(|r| r.code) != Some(*code) {
            failures.push(format!("{} ({label}): got {got:?}", code.as_str()));
            continue;
        }
        let findings = lexical_findings(input);
        if *lexical {
            if !findings.contains(*code) {
                failures.push(format!(
                    "{} ({label}): not in lexical_findings",
                    code.as_str()
                ));
            }
        } else if !findings.is_empty() {
            failures.push(format!(
                "{} ({label}): lexical findings {findings:?}",
                code.as_str()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} plant failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    for code in RefusalCode::CODES {
        assert!(
            plants.iter().any(|p| p.code == code),
            "no plant for {}",
            code.as_str()
        );
    }
}
