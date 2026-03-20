use designer_agent::component_lib::{generate_library, BrandGuide};
use designer_agent::generator::{generate_design, DesignComponent, LayoutKind, LayoutNode};
use designer_agent::screenshot_to_code::screenshot_to_code;

#[test]
#[ignore = "requires Ollama with a deployed model"]
fn test_design_generation() {
    let spec = generate_design("Dashboard for analytics app").expect("design generation succeeds");

    assert!(
        contains_kind(&spec.layout_tree, LayoutKind::Sidebar),
        "expected sidebar layout"
    );
    assert!(
        contains_kind(&spec.layout_tree, LayoutKind::CardGrid),
        "expected card grid layout"
    );
    assert!(
        has_component(&spec.components, "LineChartPanel")
            || has_component(&spec.components, "BarChartPanel"),
        "expected chart component"
    );
}

#[test]
fn test_component_library() {
    let brand = BrandGuide {
        brand_name: "BlueOrbit".to_string(),
        primary_color: "#2563EB".to_string(),
        secondary_color: "#1D4ED8".to_string(),
        neutral_color: "#F8FAFC".to_string(),
        spacing_token: "p-4".to_string(),
    };
    let library = generate_library(&brand);

    assert!(library.components.len() >= 10);
    assert!(
        library
            .components
            .iter()
            .all(|component| component.react_tsx.contains("#2563EB")),
        "all components should carry primary color"
    );
    assert!(
        library
            .components
            .iter()
            .all(|component| component.react_tsx.contains("p-4")),
        "all components should use consistent spacing token"
    );
}

#[test]
#[ignore = "requires Ollama with a deployed model"]
fn test_screenshot_analysis() {
    let result = screenshot_to_code(
        "card component with padding 24px white background radius 12px medium shadow title and body text",
    )
    .expect("screenshot analysis should succeed");

    assert_eq!(result.analysis.padding_px, 24);
    assert_eq!(result.analysis.background_color, "#FFFFFF");
    assert_eq!(result.analysis.border_radius_px, 12);
    assert_eq!(result.analysis.shadow, "medium");
    assert!(result
        .analysis
        .text_hierarchy
        .contains(&"title".to_string()));
    assert!(result.analysis.text_hierarchy.contains(&"body".to_string()));
}

fn contains_kind(node: &LayoutNode, kind: LayoutKind) -> bool {
    if node.kind == kind {
        return true;
    }
    node.children.iter().any(|child| contains_kind(child, kind))
}

fn has_component(components: &[DesignComponent], name: &str) -> bool {
    components.iter().any(|component| component.name == name)
}
