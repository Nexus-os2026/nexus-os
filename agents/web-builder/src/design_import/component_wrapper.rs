//! Component Wrapper — wrap detected sections as React components.
//!
//! Generates a complete ReactProject from imported sections.

use super::section_detector::DetectedSection;
use super::ImportError;
use crate::react_gen::{ReactProject, ReactProjectFile};
use crate::token_tailwind;
use crate::tokens::TokenSet;
use std::fmt::Write;

/// Generate a complete React project from imported sections.
pub fn generate_components_from_import(
    sections: &[DetectedSection],
    tokens: &TokenSet,
    project_name: &str,
) -> Result<ReactProject, ImportError> {
    let mut files = vec![
        generate_package_json(project_name),
        generate_tsconfig(),
        generate_vite_config(),
        generate_tailwind_config(tokens),
        generate_postcss_config(),
        generate_index_html(project_name),
        generate_main_tsx(),
        generate_index_css(tokens),
    ];

    // Section components
    let mut component_imports = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        let component_name = to_component_name(&section.suggested_id, i);
        let file = generate_section_component(&component_name, section);
        component_imports.push((component_name.clone(), section.suggested_id.clone()));
        files.push(file);
    }

    // App.tsx
    files.push(generate_app_tsx(&component_imports));

    Ok(ReactProject {
        files,
        project_name: sanitize_project_name(project_name),
        template_id: "imported".into(),
    })
}

fn to_component_name(suggested_id: &str, index: usize) -> String {
    let base = suggested_id
        .split('_')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<String>();

    if base.is_empty() {
        format!("Section{index}")
    } else {
        format!("{base}Section")
    }
}

fn sanitize_project_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn generate_section_component(component_name: &str, section: &DetectedSection) -> ReactProjectFile {
    let mut tsx = String::with_capacity(512);

    let _ = writeln!(tsx, "export default function {component_name}() {{");
    let _ = writeln!(tsx, "  return (");
    let _ = writeln!(
        tsx,
        "    <section data-nexus-section=\"{}\" data-nexus-editable=\"true\" className=\"bg-section-bg text-section-text py-section\">",
        section.suggested_id
    );
    let _ = writeln!(tsx, "      <div className=\"max-w-7xl mx-auto px-6\">");

    // Inject the sanitized HTML fragment
    // Use dangerouslySetInnerHTML since the content is already sanitized
    let escaped_html = section
        .html_fragment
        .replace('`', "\\`")
        .replace("${", "\\${");
    let _ = writeln!(
        tsx,
        "        <div dangerouslySetInnerHTML={{{{ __html: `{escaped_html}` }}}} />"
    );

    let _ = writeln!(tsx, "      </div>");
    let _ = writeln!(tsx, "    </section>");
    let _ = writeln!(tsx, "  )");
    let _ = writeln!(tsx, "}}");

    ReactProjectFile {
        path: format!("src/components/{component_name}.tsx"),
        content: tsx,
    }
}

fn generate_app_tsx(components: &[(String, String)]) -> ReactProjectFile {
    let mut tsx = String::with_capacity(512);

    for (name, _) in components {
        let _ = writeln!(tsx, "import {name} from './components/{name}'");
    }
    let _ = writeln!(tsx);
    let _ = writeln!(tsx, "export default function App() {{");
    let _ = writeln!(tsx, "  return (");
    let _ = writeln!(
        tsx,
        "    <div className=\"min-h-screen bg-bg text-text-primary\">"
    );
    for (name, _) in components {
        let _ = writeln!(tsx, "      <{name} />");
    }
    let _ = writeln!(tsx, "    </div>");
    let _ = writeln!(tsx, "  )");
    let _ = writeln!(tsx, "}}");

    ReactProjectFile {
        path: "src/App.tsx".into(),
        content: tsx,
    }
}

fn generate_package_json(project_name: &str) -> ReactProjectFile {
    let name = sanitize_project_name(project_name);
    ReactProjectFile {
        path: "package.json".into(),
        content: format!(
            r#"{{
  "name": "{name}",
  "private": true,
  "version": "0.0.1",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  }},
  "devDependencies": {{
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "autoprefixer": "^10.4.20",
    "postcss": "^8.4.49",
    "tailwindcss": "^3.4.0",
    "typescript": "~5.6.0",
    "vite": "^6.0.0"
  }}
}}"#
        ),
    }
}

fn generate_tsconfig() -> ReactProjectFile {
    ReactProjectFile {
        path: "tsconfig.json".into(),
        content: r#"{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}"#
        .into(),
    }
}

fn generate_vite_config() -> ReactProjectFile {
    ReactProjectFile {
        path: "vite.config.ts".into(),
        content: r#"import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
})
"#
        .into(),
    }
}

fn generate_tailwind_config(tokens: &TokenSet) -> ReactProjectFile {
    ReactProjectFile {
        path: "tailwind.config.ts".into(),
        content: token_tailwind::token_set_to_tailwind_config(tokens),
    }
}

fn generate_postcss_config() -> ReactProjectFile {
    ReactProjectFile {
        path: "postcss.config.js".into(),
        content: r#"export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
}
"#
        .into(),
    }
}

fn generate_index_html(project_name: &str) -> ReactProjectFile {
    ReactProjectFile {
        path: "index.html".into(),
        content: format!(
            r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{project_name}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
        ),
    }
}

fn generate_main_tsx() -> ReactProjectFile {
    ReactProjectFile {
        path: "src/main.tsx".into(),
        content: r#"import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import './index.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
"#
        .into(),
    }
}

fn generate_index_css(tokens: &TokenSet) -> ReactProjectFile {
    ReactProjectFile {
        path: "src/index.css".into(),
        content: token_tailwind::token_set_to_index_css(tokens),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sections() -> Vec<DetectedSection> {
        vec![
            DetectedSection {
                element: "header".into(),
                suggested_id: "nav".into(),
                content_summary: "Navigation".into(),
                html_fragment: "<nav><a href='/'>Home</a></nav>".into(),
            },
            DetectedSection {
                element: "section".into(),
                suggested_id: "hero".into(),
                content_summary: "Hero banner".into(),
                html_fragment: "<h1>Welcome</h1><p>Hello world</p>".into(),
            },
            DetectedSection {
                element: "footer".into(),
                suggested_id: "footer".into(),
                content_summary: "Copyright".into(),
                html_fragment: "<p>Copyright 2026</p>".into(),
            },
        ]
    }

    #[test]
    fn test_generates_react_components() {
        let tokens = TokenSet::default();
        let project = generate_components_from_import(&sample_sections(), &tokens, "test").unwrap();
        // Should have section components
        let component_files: Vec<_> = project
            .files
            .iter()
            .filter(|f| f.path.starts_with("src/components/"))
            .collect();
        assert_eq!(component_files.len(), 3, "should have 3 section components");
    }

    #[test]
    fn test_components_have_data_nexus_section() {
        let tokens = TokenSet::default();
        let project = generate_components_from_import(&sample_sections(), &tokens, "test").unwrap();
        for f in &project.files {
            if f.path.starts_with("src/components/") && f.path.ends_with("Section.tsx") {
                assert!(
                    f.content.contains("data-nexus-section"),
                    "{} should have data-nexus-section",
                    f.path
                );
            }
        }
    }

    #[test]
    fn test_generates_full_project() {
        let tokens = TokenSet::default();
        let project = generate_components_from_import(&sample_sections(), &tokens, "test").unwrap();
        let paths: Vec<&str> = project.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"package.json"));
        assert!(paths.contains(&"tsconfig.json"));
        assert!(paths.contains(&"vite.config.ts"));
        assert!(paths.contains(&"tailwind.config.ts"));
        assert!(paths.contains(&"src/main.tsx"));
        assert!(paths.contains(&"src/index.css"));
        assert!(paths.contains(&"src/App.tsx"));
    }

    #[test]
    fn test_components_use_token_classes() {
        let tokens = TokenSet::default();
        let project = generate_components_from_import(&sample_sections(), &tokens, "test").unwrap();
        for f in &project.files {
            if f.path.starts_with("src/components/") && f.path.ends_with("Section.tsx") {
                assert!(
                    f.content.contains("bg-section-bg") || f.content.contains("text-section-text"),
                    "{} should use token-based Tailwind classes",
                    f.path
                );
            }
        }
    }

    #[test]
    fn test_app_tsx_imports_all_components() {
        let tokens = TokenSet::default();
        let project = generate_components_from_import(&sample_sections(), &tokens, "test").unwrap();
        let app = project
            .files
            .iter()
            .find(|f| f.path == "src/App.tsx")
            .unwrap();
        assert!(app.content.contains("NavSection"));
        assert!(app.content.contains("HeroSection"));
        assert!(app.content.contains("FooterSection"));
    }
}
