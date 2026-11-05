//! D3 oracle amendment. Old: "selected textual cfg prefixes imply test-only".
//! New: "exclude only a parsed boolean predicate proved false whenever test=false".

use super::{cfg_test_items, mask};

fn items(src: &str) -> (super::Ranges, super::ModuleDecls) {
    cfg_test_items(&mask(src, false), &mask(src, true))
}

#[test]
fn cfg_reviewer_production_bypass_is_retained_on_every_item_shape() {
    // Exact review predicate: enabled on Unix even when `test` is false.
    for item in [
        "pub fn name_is_cloud() -> bool { false }",
        "pub const P: &str = \"cloud\";",
        "impl ModelTransport for Alternate {}",
        "mod alternate { fn name_is_cloud() {} }",
        "mod alternate;",
    ] {
        let src = format!("#[cfg(all(any(test, unix)))]\n{item}\n");
        let (ranges, modules) = items(&src);
        assert!(ranges.is_empty(), "production item disappeared: {src}");
        assert!(modules.is_empty(), "production module disappeared: {src}");
    }
}

#[test]
fn cfg_boolean_predicates_keep_every_possible_production_branch() {
    for predicate in [
        "any(test, unix)",
        "all(any(test, unix))",
        "all(any(test, unix), feature = \"ordinary\")",
        "not(test)",
        "not(all(test, unix))",
        "any(not(unix), unix)",
        "any(not(not(test)), unix)",
        "all()",
        "testimony",
        "all(testimony, unix)",
        "any(test, target_os = \"linux\")",
        "all(any(test, unix), not(feature = \"disabled\"))",
    ] {
        let src = format!("#[cfg({predicate})] fn name_is_cloud() {{}}\n");
        assert!(
            items(&src).0.is_empty(),
            "production predicate: {predicate}"
        );
    }
}

#[test]
fn cfg_boolean_predicates_exclude_only_proved_test_requirements() {
    for predicate in [
        "test",
        "all(test, unix)",
        "all(unix, test)",
        "all(any(test, unix), test)",
        "any(all(test, unix), all(test, windows))",
        "not(not(test))",
        "not(any(not(test), unix))",
        "all(any(test, unix), not(unix))",
        "all(test, feature = \"ordinary\",)",
        "any()",
    ] {
        for item in [
            "fn name_is_cloud() {}",
            "impl ModelTransport for T {}",
            "mod alternate;",
        ] {
            let src = format!("#[cfg ( {predicate} )]\n{item}\nfn production() {{}}\n");
            let (ranges, modules) = items(&src);
            assert_eq!(ranges.len(), 1, "test requirement: {predicate} on {item}");
            assert!(src[ranges[0].0..ranges[0].1].contains(item));
            assert!(!src[ranges[0].0..ranges[0].1].contains("fn production"));
            assert_eq!(modules.len(), usize::from(item.starts_with("mod ")));
        }
    }
}

#[test]
fn cfg_uncertain_or_unparseable_predicates_remain_production() {
    for predicate in [
        "all(test unix)",
        "all(test) trailing",
        "not(test, unix)",
        "not()",
        "mystery(test)",
        "all(test, mystery(unix))",
        "test = 1",
        "all(test, feature = ordinary)",
        "all(test, any(test, unix)",
    ] {
        let src = format!("#[cfg({predicate})] fn name_is_cloud() {{}}\n");
        assert!(
            items(&src).0.is_empty(),
            "uncertain predicate disappeared: {predicate}"
        );
    }
    assert_eq!(items("#[cfg(all(test,))] fn genuine_test() {}").0.len(), 1);
}

#[test]
fn cfg_attr_only_excludes_when_its_conditional_effect_requires_test() {
    for attribute in [
        "#[cfg_attr(test, cfg(test))]",
        "#[cfg_attr(unix, cfg(test))]",
        "#[cfg_attr(test, allow(dead_code))]",
        "#[cfg_attr(all(any(test, unix)), cfg(test))]",
        "#[cfg_attr(unix, cfg(test), allow(dead_code))]",
        "#[cfg_attr(test, cfg_attr(unix, cfg(test)))]",
        "#[cfg_attr(mystery(test), cfg(test))]",
    ] {
        let src = format!("{attribute}\nfn name_is_cloud() {{}}\n");
        assert!(items(&src).0.is_empty(), "production cfg_attr: {attribute}");
    }
    for attribute in [
        "#[cfg_attr(not(test), cfg(test))]",
        "#[cfg_attr(all(), cfg(test))]",
        "#[cfg_attr(not(test), cfg_attr(not(test), cfg(test)))]",
    ] {
        let src = format!("{attribute}\nimpl ModelTransport for Alternate {{}}\n");
        assert_eq!(items(&src).0.len(), 1, "test-only cfg_attr: {attribute}");
    }
}

#[test]
fn cfg_modules_retain_paths_and_multiple_outer_attributes() {
    let src = "#[cfg(all(test, unix))]\n#[path = \"fixture.rs\"]\n#[allow(dead_code)]\nmod fixture;\nfn production() {}\n";
    let (ranges, modules) = items(src);
    assert_eq!(ranges.len(), 1);
    assert_eq!(
        modules,
        [("fixture".to_string(), Some("fixture.rs".to_string()))]
    );
    assert!(!src[ranges[0].0..ranges[0].1].contains("fn production"));

    let production = "#[cfg(all(any(test, unix)))]\n#[path = \"fixture.rs\"]\nmod fixture;";
    assert!(items(production).0.is_empty());
    assert!(items(production).1.is_empty());
}

#[test]
fn cfg_whitespace_and_literal_valued_atoms_preserve_boolean_logic() {
    for src in [
        "# [ cfg ( all ( test , unix ) ) ] fn test_only() {}",
        r##"#[cfg(all(test, feature = r#"ordinary"#))] fn test_only() {}"##,
    ] {
        assert_eq!(items(src).0.len(), 1, "proved test-only: {src}");
    }
    for src in [
        "# [ cfg ( all ( any ( test , unix ) ) ) ] fn production() {}",
        r##"#[cfg(any(test, feature = r#"ordinary"#))] fn production() {}"##,
    ] {
        assert!(items(src).0.is_empty(), "possible production: {src}");
    }
}
