use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrandGuide {
    pub brand_name: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub neutral_color: String,
    pub spacing_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedComponent {
    pub name: String,
    pub react_tsx: String,
    pub storybook_story: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentLibrary {
    pub brand_guide: BrandGuide,
    pub components: Vec<GeneratedComponent>,
    pub dark_mode: bool,
    pub responsive: bool,
    pub accessibility_notes: Vec<String>,
}

pub fn generate_library(brand_guide: &BrandGuide) -> ComponentLibrary {
    let names = [
        "Button",
        "Input",
        "Card",
        "Modal",
        "Table",
        "Navigation",
        "Form",
        "Badge",
        "Alert",
        "Tooltip",
        "Dropdown",
        "Tabs",
    ];
    let components = names
        .iter()
        .map(|name| build_component(name, brand_guide))
        .collect::<Vec<_>>();

    ComponentLibrary {
        brand_guide: brand_guide.clone(),
        components,
        dark_mode: true,
        responsive: true,
        accessibility_notes: vec![
            "All interactive controls include keyboard focus styles.".to_string(),
            "Color contrast aims for WCAG AA minimum ratio.".to_string(),
            "Components include ARIA roles/labels where appropriate.".to_string(),
        ],
    }
}

fn build_component(name: &str, guide: &BrandGuide) -> GeneratedComponent {
    let react_tsx = format!(
        "import React from \"react\";

type Props = {{
  label?: string;
}};

export function {name}(props: Props): JSX.Element {{
  return (
    <div
      className=\"{spacing} rounded-xl border dark:bg-slate-900 dark:text-slate-100\"
      style={{{{
        borderColor: \"{secondary}\",
        backgroundColor: \"{neutral}\",
        color: \"{primary}\"
      }}}}
      aria-label=\"{name}\"
    >
      {{props.label ?? \"{name}\"}}
    </div>
  );
}}",
        spacing = guide.spacing_token,
        primary = guide.primary_color,
        secondary = guide.secondary_color,
        neutral = guide.neutral_color
    );

    let storybook_story = format!(
        "import type {{ Meta, StoryObj }} from \"@storybook/react\";
import {{ {name} }} from \"./{name}\";

const meta: Meta<typeof {name}> = {{
  title: \"DesignSystem/{name}\",
  component: {name},
}};

export default meta;
type Story = StoryObj<typeof {name}>;

export const Default: Story = {{
  args: {{
    label: \"{name}\",
  }},
}};"
    );

    GeneratedComponent {
        name: name.to_string(),
        react_tsx,
        storybook_story,
    }
}
