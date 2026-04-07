//! React Project Generator — transforms ContentPayload + tokens into a Vite + React + TS project.
//!
//! Two-phase generation:
//! 1. **Scaffold phase** (deterministic, $0): project structure, configs, typed components
//! 2. **Enhancement phase** (LLM, optional): complex interactivity via gemma4:e4b or Sonnet
//!
//! The scaffold phase works for ALL 6 templates without any LLM call.

use crate::content_payload::ContentPayload;
use crate::react_components::{generate_section_component, ProjectFile};
use crate::slot_schema::TemplateSchema;
use crate::token_tailwind::{token_set_to_index_css, token_set_to_tailwind_config};
use crate::tokens::TokenSet;
use crate::variant::VariantSelection;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ReactGenError {
    #[error("template schema not found: {0}")]
    SchemaNotFound(String),
    #[error("component generation failed: {0}")]
    ComponentError(String),
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// The complete generated React project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactProject {
    pub files: Vec<ReactProjectFile>,
    pub project_name: String,
    pub template_id: String,
}

/// Serializable version of ProjectFile for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactProjectFile {
    pub path: String,
    pub content: String,
}

impl From<ProjectFile> for ReactProjectFile {
    fn from(f: ProjectFile) -> Self {
        ReactProjectFile {
            path: f.path,
            content: f.content,
        }
    }
}

/// Page specification for multi-page apps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSpec {
    pub name: String,
    pub route: String,
    pub sections: Vec<String>, // section IDs to include on this page
}

/// Output mode for the builder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputMode {
    #[default]
    Html,
    React,
}

// ─── Output Mode Selection ──────────────────────────────────────────────────

/// Determine the output mode from the user's brief.
pub fn detect_output_mode(brief: &str) -> OutputMode {
    let lower = brief.to_lowercase();
    let react_signals = [
        "app",
        "dashboard",
        "multi-page",
        "multipage",
        "login",
        "routing",
        "react",
        "typescript",
        "components",
        "spa",
        "single page app",
        "admin panel",
        "settings page",
        "authentication",
    ];
    let html_signals = [
        "landing page",
        "portfolio",
        "single page",
        "one page",
        "static site",
        "simple website",
        "html",
        "brochure",
    ];

    let react_score: usize = react_signals.iter().filter(|s| lower.contains(*s)).count();
    let html_score: usize = html_signals.iter().filter(|s| lower.contains(*s)).count();

    if react_score > html_score {
        OutputMode::React
    } else {
        OutputMode::Html
    }
}

// ─── Scaffold Generation ────────────────────────────────────────────────────

/// Generate a complete React project scaffold (deterministic, $0).
pub fn generate_react_project(
    payload: &ContentPayload,
    schema: &TemplateSchema,
    variant: &VariantSelection,
    token_set: &TokenSet,
    project_name: &str,
    pages: Option<Vec<PageSpec>>,
) -> Result<ReactProject, ReactGenError> {
    let mut files: Vec<ReactProjectFile> = Vec::new();
    let is_multi_page = pages.is_some();
    let pages = pages.unwrap_or_default();
    let clean_name = project_name
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();

    // 1. package.json
    files.push(ReactProjectFile {
        path: "package.json".into(),
        content: generate_package_json(&clean_name, is_multi_page),
    });

    // 2. tsconfig.json
    files.push(ReactProjectFile {
        path: "tsconfig.json".into(),
        content: generate_tsconfig(),
    });

    // 3. vite.config.ts
    files.push(ReactProjectFile {
        path: "vite.config.ts".into(),
        content: generate_vite_config(),
    });

    // 4. tailwind.config.ts
    files.push(ReactProjectFile {
        path: "tailwind.config.ts".into(),
        content: token_set_to_tailwind_config(token_set),
    });

    // 5. postcss.config.js
    files.push(ReactProjectFile {
        path: "postcss.config.js".into(),
        content:
            "export default {\n  plugins: {\n    tailwindcss: {},\n    autoprefixer: {},\n  },\n}\n"
                .into(),
    });

    // 6. index.html
    files.push(ReactProjectFile {
        path: "index.html".into(),
        content: generate_index_html(&clean_name),
    });

    // 7. public/favicon.svg
    files.push(ReactProjectFile {
        path: "public/favicon.svg".into(),
        content: "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 32 32\"><rect width=\"32\" height=\"32\" rx=\"6\" fill=\"#6366f1\"/><text x=\"16\" y=\"22\" text-anchor=\"middle\" fill=\"white\" font-size=\"18\" font-family=\"system-ui\">N</text></svg>".into(),
    });

    // 8. src/main.tsx
    files.push(ReactProjectFile {
        path: "src/main.tsx".into(),
        content: generate_main_tsx(is_multi_page),
    });

    // 9. src/index.css
    files.push(ReactProjectFile {
        path: "src/index.css".into(),
        content: token_set_to_index_css(token_set),
    });

    // 10. Section components
    let mut section_imports = Vec::new();
    for section_schema in &schema.sections {
        let content = payload
            .sections
            .iter()
            .find(|s| s.section_id == section_schema.section_id);
        let layout = variant
            .layout
            .get(&section_schema.section_id)
            .map(|s| s.as_str())
            .unwrap_or("default");

        let file = generate_section_component(section_schema, content, layout, &schema.template_id)
            .map_err(|e| ReactGenError::ComponentError(e.to_string()))?;

        let component_name = pascal_case(&section_schema.section_id) + "Section";
        section_imports.push((
            component_name.clone(),
            file.path.clone(),
            section_schema.section_id.clone(),
            content.is_some(),
        ));
        files.push(file.into());
    }

    // 11. Layout component
    files.push(ReactProjectFile {
        path: "src/components/Layout.tsx".into(),
        content: generate_layout_tsx(&section_imports, project_name),
    });

    // 12. App.tsx
    files.push(ReactProjectFile {
        path: "src/App.tsx".into(),
        content: if is_multi_page {
            generate_multi_page_app_tsx(&pages, &section_imports)
        } else {
            generate_single_page_app_tsx(&section_imports)
        },
    });

    // 13. Page files for multi-page apps
    if is_multi_page {
        for page in &pages {
            files.push(ReactProjectFile {
                path: format!("src/pages/{}.tsx", pascal_case(&page.name)),
                content: generate_page_tsx(page, &section_imports),
            });
        }
    } else {
        // Single-page: Home.tsx
        files.push(ReactProjectFile {
            path: "src/pages/Home.tsx".into(),
            content: generate_home_page_tsx(&section_imports),
        });
    }

    // 14. Utils
    files.push(ReactProjectFile {
        path: "src/lib/utils.ts".into(),
        content: "export function cn(...classes: (string | undefined | false)[]): string {\n  return classes.filter(Boolean).join(' ')\n}\n".into(),
    });

    // 15. README.md
    files.push(ReactProjectFile {
        path: "README.md".into(),
        content: generate_readme(project_name, &schema.template_id, &payload.variant),
    });

    Ok(ReactProject {
        files,
        project_name: clean_name,
        template_id: schema.template_id.clone(),
    })
}

// ─── File Generators ────────────────────────────────────────────────────────

fn pascal_case(s: &str) -> String {
    s.split('_')
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

fn generate_package_json(name: &str, has_router: bool) -> String {
    let router_dep = if has_router {
        "\n    \"react-router-dom\": \"^6.23.1\","
    } else {
        ""
    };
    format!(
        r#"{{
  "name": "{name}",
  "private": true,
  "version": "1.0.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "react": "^18.3.1",
    "react-dom": "^18.3.1"{router_dep}
  }},
  "devDependencies": {{
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "autoprefixer": "^10.4.19",
    "postcss": "^8.4.38",
    "tailwindcss": "^3.4.4",
    "typescript": "^5.5.3",
    "vite": "^5.3.4"
  }}
}}
"#
    )
}

fn generate_tsconfig() -> String {
    r#"{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}
"#
    .into()
}

fn generate_vite_config() -> String {
    r#"import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
})
"#
    .into()
}

fn generate_index_html(title: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{title}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
    )
}

fn generate_main_tsx(has_router: bool) -> String {
    if has_router {
        r#"import React from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import App from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
)
"#
        .into()
    } else {
        r#"import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
"#
        .into()
    }
}

fn generate_single_page_app_tsx(_sections: &[(String, String, String, bool)]) -> String {
    let mut tsx = String::from("import Home from './pages/Home'\n\n");
    tsx.push_str("export default function App() {\n");
    tsx.push_str("  return <Home />\n");
    tsx.push_str("}\n");
    tsx
}

fn generate_multi_page_app_tsx(
    pages: &[PageSpec],
    _sections: &[(String, String, String, bool)],
) -> String {
    let mut tsx = String::new();
    tsx.push_str("import { Routes, Route } from 'react-router-dom'\n");
    tsx.push_str("import Layout from './components/Layout'\n");
    for page in pages {
        let name = pascal_case(&page.name);
        let _ = writeln!(tsx, "import {name} from './pages/{name}'");
    }
    tsx.push('\n');
    tsx.push_str("export default function App() {\n");
    tsx.push_str("  return (\n");
    tsx.push_str("    <Layout>\n");
    tsx.push_str("      <Routes>\n");
    for page in pages {
        let name = pascal_case(&page.name);
        let _ = writeln!(
            tsx,
            "        <Route path=\"{}\" element={{<{name} />}} />",
            page.route
        );
    }
    tsx.push_str("      </Routes>\n");
    tsx.push_str("    </Layout>\n");
    tsx.push_str("  )\n");
    tsx.push_str("}\n");
    tsx
}

fn generate_layout_tsx(_sections: &[(String, String, String, bool)], project_name: &str) -> String {
    let mut tsx = String::new();
    tsx.push_str("interface LayoutProps {\n");
    tsx.push_str("  children: React.ReactNode\n");
    tsx.push_str("}\n\n");
    tsx.push_str("export default function Layout({ children }: LayoutProps) {\n");
    tsx.push_str("  return (\n");
    tsx.push_str("    <div className=\"min-h-screen bg-bg text-text-primary\">\n");
    // Nav
    let _ = writeln!(
        tsx,
        "      <nav className=\"sticky top-0 z-50 bg-nav-bg/80 backdrop-blur border-b border-border\" data-nexus-section=\"nav\">"
    );
    let _ = writeln!(
        tsx,
        "        <div className=\"max-w-7xl mx-auto px-6 py-md flex items-center justify-between\">"
    );
    let _ = writeln!(
        tsx,
        "          <span className=\"font-heading font-bold text-xl\">{project_name}</span>"
    );
    tsx.push_str("          <div className=\"flex gap-lg text-sm text-text-secondary\">\n");
    tsx.push_str(
        "            <a href=\"#\" className=\"hover:text-primary transition-colors\">Home</a>\n",
    );
    tsx.push_str(
        "            <a href=\"#\" className=\"hover:text-primary transition-colors\">About</a>\n",
    );
    tsx.push_str("            <a href=\"#\" className=\"hover:text-primary transition-colors\">Contact</a>\n");
    tsx.push_str("          </div>\n");
    tsx.push_str("        </div>\n");
    tsx.push_str("      </nav>\n");
    // Main content
    tsx.push_str("      <main>{children}</main>\n");
    // Footer
    tsx.push_str("      <footer className=\"bg-footer-bg text-footer-text py-xl border-t border-border\" data-nexus-section=\"footer\">\n");
    tsx.push_str("        <div className=\"max-w-7xl mx-auto px-6 text-center text-sm\">\n");
    let _ = writeln!(
        tsx,
        "          <p>&copy; 2026 {project_name}. All rights reserved.</p>"
    );
    tsx.push_str("        </div>\n");
    tsx.push_str("      </footer>\n");
    tsx.push_str("    </div>\n");
    tsx.push_str("  )\n");
    tsx.push_str("}\n");
    tsx
}

fn generate_home_page_tsx(sections: &[(String, String, String, bool)]) -> String {
    let mut tsx = String::new();
    // Import all section components (skip footer — handled by Layout)
    for (name, path, section_id, _) in sections {
        if section_id == "footer" {
            continue;
        }
        let rel_path = path.strip_prefix("src/").unwrap_or(path);
        let rel_path = rel_path.strip_suffix(".tsx").unwrap_or(rel_path);
        let _ = writeln!(
            tsx,
            "import {name}, {{ default{name}Props }} from '../{rel_path}'"
        );
    }
    tsx.push('\n');
    tsx.push_str("export default function Home() {\n");
    tsx.push_str("  return (\n");
    tsx.push_str("    <>\n");
    for (name, _, section_id, has_content) in sections {
        if section_id == "footer" {
            continue;
        }
        if *has_content {
            let _ = writeln!(tsx, "      <{name} {{...default{name}Props}} />");
        } else {
            let _ = writeln!(tsx, "      <{name} />");
        }
    }
    tsx.push_str("    </>\n");
    tsx.push_str("  )\n");
    tsx.push_str("}\n");
    tsx
}

fn generate_page_tsx(page: &PageSpec, sections: &[(String, String, String, bool)]) -> String {
    let mut tsx = String::new();
    for (name, path, section_id, _) in sections {
        if page.sections.contains(section_id) {
            let rel_path = path.strip_prefix("src/").unwrap_or(path);
            let rel_path = rel_path.strip_suffix(".tsx").unwrap_or(rel_path);
            let _ = writeln!(
                tsx,
                "import {name}, {{ default{name}Props }} from '../{rel_path}'"
            );
        }
    }
    tsx.push('\n');
    let page_name = pascal_case(&page.name);
    let _ = writeln!(tsx, "export default function {page_name}() {{");
    tsx.push_str("  return (\n");
    tsx.push_str("    <>\n");
    for (name, _, section_id, has_content) in sections {
        if page.sections.contains(section_id) {
            if *has_content {
                let _ = writeln!(tsx, "      <{name} {{...default{name}Props}} />");
            } else {
                let _ = writeln!(tsx, "      <{name} />");
            }
        }
    }
    tsx.push_str("    </>\n");
    tsx.push_str("  )\n");
    tsx.push_str("}\n");
    tsx
}

fn generate_readme(project_name: &str, template_id: &str, variant: &VariantSelection) -> String {
    format!(
        r#"# {project_name}

Built with [Nexus Builder](https://nexus-os.dev) — the governed AI app builder.

## Quick Start

```bash
npm install
npm run dev
```

## Build for Production

```bash
npm run build
```

Output in `dist/` — deploy to any static host.

## Details

- **Template:** {template_id}
- **Palette:** {}
- **Typography:** {}
- **Stack:** React 18 + TypeScript + Vite + Tailwind CSS

## Governance

This project was generated under Nexus OS governance:
- All content validated against slot schemas
- Token-driven styling (no hardcoded colors)
- Audit trail preserved
"#,
        variant.palette_id, variant.typography_id
    )
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::{ContentPayload, SectionContent};
    use crate::slot_schema::get_template_schema;
    use crate::variant::MotionProfile;
    use std::collections::HashMap;

    fn test_variant(palette: &str) -> VariantSelection {
        VariantSelection {
            palette_id: palette.into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    fn test_payload(template_id: &str) -> ContentPayload {
        ContentPayload {
            template_id: template_id.into(),
            variant: test_variant("saas_midnight"),
            sections: vec![],
        }
    }

    fn saas_payload() -> ContentPayload {
        ContentPayload {
            template_id: "saas_landing".into(),
            variant: test_variant("saas_midnight"),
            sections: vec![
                SectionContent {
                    section_id: "hero".into(),
                    slots: HashMap::from([
                        ("headline".into(), "Build Faster with AI".into()),
                        (
                            "subtitle".into(),
                            "The modern platform for developers.".into(),
                        ),
                        ("cta_primary".into(), "Start Free Trial".into()),
                    ]),
                },
                SectionContent {
                    section_id: "features".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Features".into()),
                        ("feature_1_icon".into(), "rocket".into()),
                        ("feature_1_title".into(), "Fast".into()),
                        ("feature_1_desc".into(), "Lightning fast builds.".into()),
                        ("feature_2_icon".into(), "shield".into()),
                        ("feature_2_title".into(), "Secure".into()),
                        ("feature_2_desc".into(), "Enterprise security.".into()),
                        ("feature_3_icon".into(), "chart".into()),
                        ("feature_3_title".into(), "Analytics".into()),
                        ("feature_3_desc".into(), "Real-time dashboards.".into()),
                    ]),
                },
                SectionContent {
                    section_id: "pricing".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Pricing".into()),
                        ("tier_1_name".into(), "Free".into()),
                        ("tier_1_price".into(), "$0/mo".into()),
                        (
                            "tier_1_features".into(),
                            "5 projects<br>Basic support".into(),
                        ),
                        ("tier_2_name".into(), "Pro".into()),
                        ("tier_2_price".into(), "$29/mo".into()),
                        (
                            "tier_2_features".into(),
                            "Unlimited<br>Priority support".into(),
                        ),
                        ("tier_3_name".into(), "Enterprise".into()),
                        ("tier_3_price".into(), "$99/mo".into()),
                        ("tier_3_features".into(), "Everything<br>SLA<br>SSO".into()),
                    ]),
                },
                SectionContent {
                    section_id: "testimonials".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Testimonials".into()),
                        ("testimonial_1_quote".into(), "Great tool.".into()),
                        ("testimonial_1_author".into(), "Jane".into()),
                        ("testimonial_1_role".into(), "CTO".into()),
                        ("testimonial_2_quote".into(), "Love it.".into()),
                        ("testimonial_2_author".into(), "Alex".into()),
                        ("testimonial_2_role".into(), "Engineer".into()),
                        ("testimonial_3_quote".into(), "Amazing.".into()),
                        ("testimonial_3_author".into(), "Maria".into()),
                        ("testimonial_3_role".into(), "VP Eng".into()),
                    ]),
                },
                SectionContent {
                    section_id: "cta".into(),
                    slots: HashMap::from([
                        ("headline".into(), "Ready?".into()),
                        ("body".into(), "Join now.".into()),
                        ("cta_button".into(), "Get Started".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([
                        ("brand".into(), "AcmeAI".into()),
                        ("copyright".into(), "2026 AcmeAI".into()),
                    ]),
                },
            ],
        }
    }

    #[test]
    fn test_generate_scaffold_saas_landing() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();

        let project = generate_react_project(&payload, &schema, &variant, &ts, "Acme AI", None);
        assert!(project.is_ok(), "scaffold failed: {project:?}");
        let project = project.unwrap();

        // Check expected files exist
        let paths: Vec<&str> = project.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"package.json"));
        assert!(paths.contains(&"tsconfig.json"));
        assert!(paths.contains(&"vite.config.ts"));
        assert!(paths.contains(&"tailwind.config.ts"));
        assert!(paths.contains(&"postcss.config.js"));
        assert!(paths.contains(&"index.html"));
        assert!(paths.contains(&"src/main.tsx"));
        assert!(paths.contains(&"src/index.css"));
        assert!(paths.contains(&"src/App.tsx"));
        assert!(paths.contains(&"src/components/Layout.tsx"));
        assert!(paths.contains(&"src/pages/Home.tsx"));
        assert!(paths.contains(&"README.md"));
    }

    #[test]
    fn test_generate_scaffold_all_six_templates() {
        let templates = [
            ("saas_landing", "saas_midnight"),
            ("docs_site", "docs_clean"),
            ("portfolio", "port_monochrome"),
            ("local_business", "biz_warm"),
            ("ecommerce", "ecom_luxe"),
            ("dashboard", "dash_pro"),
        ];
        for (tid, pid) in &templates {
            let schema = get_template_schema(tid).unwrap();
            let payload = test_payload(tid);
            let variant = test_variant(pid);
            let ts = variant.to_token_set().unwrap();
            let result = generate_react_project(&payload, &schema, &variant, &ts, "Test", None);
            assert!(result.is_ok(), "Failed for {tid}: {result:?}");
        }
    }

    #[test]
    fn test_scaffold_package_json_valid() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", None).unwrap();

        let pkg = project
            .files
            .iter()
            .find(|f| f.path == "package.json")
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&pkg.content).unwrap();
        assert!(parsed["dependencies"]["react"].is_string());
        assert!(parsed["devDependencies"]["typescript"].is_string());
        assert!(parsed["devDependencies"]["tailwindcss"].is_string());
    }

    #[test]
    fn test_scaffold_tsconfig_valid() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", None).unwrap();

        let tsconfig = project
            .files
            .iter()
            .find(|f| f.path == "tsconfig.json")
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig.content).unwrap();
        assert_eq!(parsed["compilerOptions"]["strict"], true);
        assert_eq!(parsed["compilerOptions"]["jsx"], "react-jsx");
    }

    #[test]
    fn test_scaffold_vite_config_valid() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", None).unwrap();

        let vite = project
            .files
            .iter()
            .find(|f| f.path == "vite.config.ts")
            .unwrap();
        assert!(vite.content.contains("defineConfig"));
        assert!(vite.content.contains("react()"));
    }

    #[test]
    fn test_scaffold_has_all_section_components() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", None).unwrap();

        for section in &schema.sections {
            let expected_path = format!(
                "src/components/{}Section.tsx",
                pascal_case(&section.section_id)
            );
            assert!(
                project.files.iter().any(|f| f.path == expected_path),
                "Missing component file: {expected_path}"
            );
        }
    }

    #[test]
    fn test_scaffold_components_have_data_nexus_section() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", None).unwrap();

        for section in &schema.sections {
            let path = format!(
                "src/components/{}Section.tsx",
                pascal_case(&section.section_id)
            );
            let file = project.files.iter().find(|f| f.path == path).unwrap();
            assert!(
                file.content
                    .contains(&format!("data-nexus-section=\"{}\"", section.section_id)),
                "Component {} missing data-nexus-section",
                section.section_id
            );
        }
    }

    #[test]
    fn test_scaffold_components_have_typed_props() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", None).unwrap();

        // Check hero component has required props
        let hero_file = project
            .files
            .iter()
            .find(|f| f.path == "src/components/HeroSection.tsx")
            .unwrap();
        assert!(
            hero_file.content.contains("headline: string"),
            "missing required prop headline"
        );
        assert!(
            hero_file.content.contains("ctaPrimary: string"),
            "missing required prop ctaPrimary"
        );
    }

    #[test]
    fn test_multi_page_generates_router() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let pages = vec![
            PageSpec {
                name: "home".into(),
                route: "/".into(),
                sections: vec!["hero".into(), "features".into()],
            },
            PageSpec {
                name: "pricing".into(),
                route: "/pricing".into(),
                sections: vec!["pricing".into()],
            },
        ];
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", Some(pages)).unwrap();

        // App.tsx should have Routes
        let app = project
            .files
            .iter()
            .find(|f| f.path == "src/App.tsx")
            .unwrap();
        assert!(
            app.content.contains("Routes"),
            "multi-page app should use Routes"
        );
        assert!(
            app.content.contains("Route"),
            "multi-page app should use Route"
        );
        assert!(app.content.contains("react-router-dom"));

        // main.tsx should have BrowserRouter
        let main = project
            .files
            .iter()
            .find(|f| f.path == "src/main.tsx")
            .unwrap();
        assert!(main.content.contains("BrowserRouter"));

        // package.json should have react-router-dom
        let pkg = project
            .files
            .iter()
            .find(|f| f.path == "package.json")
            .unwrap();
        assert!(pkg.content.contains("react-router-dom"));
    }

    #[test]
    fn test_multi_page_generates_page_files() {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = saas_payload();
        let variant = test_variant("saas_midnight");
        let ts = variant.to_token_set().unwrap();
        let pages = vec![
            PageSpec {
                name: "home".into(),
                route: "/".into(),
                sections: vec!["hero".into()],
            },
            PageSpec {
                name: "about".into(),
                route: "/about".into(),
                sections: vec!["cta".into()],
            },
        ];
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", Some(pages)).unwrap();

        assert!(
            project.files.iter().any(|f| f.path == "src/pages/Home.tsx"),
            "missing Home page"
        );
        assert!(
            project
                .files
                .iter()
                .any(|f| f.path == "src/pages/About.tsx"),
            "missing About page"
        );
    }

    #[test]
    fn test_output_mode_selection_dashboard() {
        assert_eq!(
            detect_output_mode("build me a SaaS dashboard with analytics"),
            OutputMode::React
        );
    }

    #[test]
    fn test_output_mode_selection_landing() {
        assert_eq!(
            detect_output_mode("build a landing page for my startup"),
            OutputMode::Html
        );
    }

    #[test]
    fn test_output_mode_selection_react_explicit() {
        assert_eq!(
            detect_output_mode("create a React app with login and settings"),
            OutputMode::React
        );
    }

    #[test]
    fn test_output_mode_default_html() {
        assert_eq!(
            detect_output_mode("build me a nice website"),
            OutputMode::Html
        );
    }
}
