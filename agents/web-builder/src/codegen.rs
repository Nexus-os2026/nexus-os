use crate::interpreter::{Framework, PageSpec, SectionKind, WebsiteSpec};
use crate::styles::generate_theme;
use crate::templates::default_template_engine;
use crate::threejs::{generate_3d_scene, scene_component_name};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChange {
    Create(String, String),
    Modify(String, String, String),
    Delete(String),
}

pub fn generate_website(spec: &WebsiteSpec) -> Result<Vec<FileChange>, AgentError> {
    if spec.pages.is_empty() {
        return Err(AgentError::ManifestError(
            "website spec requires at least one page".to_string(),
        ));
    }
    if spec.framework != Framework::React {
        return Err(AgentError::SupervisorError(
            "only React scaffolding is currently supported".to_string(),
        ));
    }

    let mut changes = Vec::new();
    let theme = generate_theme(spec.theme.mood.as_str(), None);
    let template_engine = default_template_engine();

    changes.push(FileChange::Create(
        "package.json".to_string(),
        package_json_content(),
    ));
    changes.push(FileChange::Create(
        "tsconfig.json".to_string(),
        tsconfig_content(),
    ));
    changes.push(FileChange::Create(
        "index.html".to_string(),
        index_html_content(),
    ));
    changes.push(FileChange::Create(
        "vite.config.ts".to_string(),
        vite_config_content(),
    ));
    changes.push(FileChange::Create(
        "tailwind.config.ts".to_string(),
        theme.tailwind_config,
    ));
    changes.push(FileChange::Create(
        "postcss.config.cjs".to_string(),
        "module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } };\n".to_string(),
    ));
    changes.push(FileChange::Create(
        "src/main.tsx".to_string(),
        "import React from 'react';\nimport ReactDOM from 'react-dom/client';\nimport App from './App';\nimport './styles/theme.css';\n\nReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(\n  <React.StrictMode>\n    <App />\n  </React.StrictMode>,\n);\n"
            .to_string(),
    ));
    changes.push(FileChange::Create(
        "src/styles/theme.css".to_string(),
        theme.css,
    ));

    let mut page_imports = Vec::new();
    let mut page_routes = Vec::new();

    for page in &spec.pages {
        let component_name = page_component_name(page.name.as_str());
        page_imports.push(format!(
            "import {component_name} from './pages/{component_name}';"
        ));
        page_routes.push(format!(
            "  {{ key: '{}', label: '{}', component: {component_name} }},",
            slug(page.name.as_str()),
            page.name
        ));

        let page_content = render_page_component(page, &template_engine);
        changes.push(FileChange::Create(
            format!("src/pages/{component_name}.tsx"),
            page_content,
        ));
    }

    for element in &spec.three_d_elements {
        let component_name = scene_component_name(element.model.as_str());
        let scene_source = generate_3d_scene(element);
        changes.push(FileChange::Create(
            format!("src/components/scenes/{component_name}.tsx"),
            scene_source,
        ));
    }

    let app_tsx = format!(
        "import React, {{ useMemo, useState }} from 'react';\n\
{imports}\n\
\n\
type RouteDef = {{ key: string; label: string; component: () => JSX.Element }};\n\
\n\
const routes: RouteDef[] = [\n{routes}\n];\n\
\n\
export default function App(): JSX.Element {{\n\
  const [active, setActive] = useState<string>(routes[0]?.key ?? 'home');\n\
  const current = useMemo(() => routes.find((route) => route.key === active) ?? routes[0], [active]);\n\
  const CurrentComponent = current.component;\n\
\n\
  return (\n\
    <div className=\"min-h-screen bg-bg text-text\">\n\
      <header className=\"sticky top-0 z-20 border-b border-white/10 bg-surface/80 backdrop-blur\">\n\
        <nav aria-label=\"page navigation\" className=\"mx-auto flex max-w-6xl gap-2 px-4 py-3\">\n\
          {{routes.map((route) => (\n\
            <button\n\
              key={{route.key}}\n\
              type=\"button\"\n\
              onClick={{() => setActive(route.key)}}\n\
              className={{`rounded-full px-4 py-2 text-sm transition-colors ${{active === route.key ? 'bg-accent text-black' : 'bg-black/20 text-white'}}`}}\n\
            >\n\
              {{route.label}}\n\
            </button>\n\
          ))}}\n\
        </nav>\n\
      </header>\n\
\n\
      <main className=\"mx-auto max-w-6xl px-4 py-8\">\n\
        <CurrentComponent />\n\
      </main>\n\
    </div>\n\
  );\n\
}}\n",
        imports = page_imports.join("\n"),
        routes = page_routes.join("\n"),
    );

    changes.push(FileChange::Create("src/App.tsx".to_string(), app_tsx));

    Ok(changes)
}

fn package_json_content() -> String {
    r#"{
  "name": "nexus-web-builder-output",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "@react-three/fiber": "^8.17.10",
    "@react-three/drei": "^9.111.3",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "three": "^0.169.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "autoprefixer": "^10.4.20",
    "postcss": "^8.4.49",
    "tailwindcss": "^3.4.14",
    "typescript": "^5.6.3",
    "vite": "^5.4.10"
  }
}
"#
    .to_string()
}

fn tsconfig_content() -> String {
    r#"{
  "compilerOptions": {
    "target": "ES2021",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "jsx": "react-jsx",
    "strict": true,
    "types": ["vite/client"],
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["src", "vite.config.ts", "tailwind.config.ts"]
}
"#
    .to_string()
}

fn index_html_content() -> String {
    "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n    <title>Nexus Web Builder</title>\n  </head>\n  <body>\n    <div id=\"root\"></div>\n    <script type=\"module\" src=\"/src/main.tsx\"></script>\n  </body>\n</html>\n"
        .to_string()
}

fn vite_config_content() -> String {
    "import { defineConfig } from 'vite';\n\nexport default defineConfig({\n  server: {\n    host: '127.0.0.1',\n    port: 5173,\n  },\n});\n"
        .to_string()
}

fn render_page_component(page: &PageSpec, templates: &crate::templates::TemplateEngine) -> String {
    let component_name = page_component_name(page.name.as_str());
    let mut rendered_sections = Vec::new();
    for section in &page.sections {
        let title = format!("{} Section", section_label(&section.kind));
        let body = section.content.as_str();
        if let Some(rendered) =
            templates.render_component(section.template_id.as_str(), title.as_str(), body)
        {
            rendered_sections.push(rendered);
        } else {
            rendered_sections.push(format!(
                "<section aria-label=\"{}\" className=\"rounded-2xl border border-white/10 p-6\"><h2 className=\"text-2xl font-display\">{}</h2><p className=\"mt-2\">{}</p></section>",
                section_label(&section.kind).to_ascii_lowercase(),
                title,
                body
            ));
        }
    }

    format!(
        "import React from 'react';\n\
\n\
export default function {component_name}(): JSX.Element {{\n\
  return (\n\
    <div className=\"space-y-8\" data-layout=\"{layout}\">\n\
      {sections}\n\
    </div>\n\
  );\n\
}}\n",
        component_name = component_name,
        layout = page.layout,
        sections = rendered_sections.join("\n      "),
    )
}

fn page_component_name(name: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper_next {
                out.push(ch.to_ascii_uppercase());
                upper_next = false;
            } else {
                out.push(ch.to_ascii_lowercase());
            }
        } else {
            upper_next = true;
        }
    }
    if out.is_empty() {
        out.push_str("Page");
    }
    format!("{}Page", out)
}

fn slug(input: &str) -> String {
    let mut slug = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

fn section_label(kind: &SectionKind) -> String {
    match kind {
        SectionKind::Header => "Header".to_string(),
        SectionKind::Hero => "Hero".to_string(),
        SectionKind::Features => "Features".to_string(),
        SectionKind::Testimonials => "Testimonials".to_string(),
        SectionKind::Pricing => "Pricing".to_string(),
        SectionKind::Menu => "Menu".to_string(),
        SectionKind::Contact => "Contact".to_string(),
        SectionKind::Footer => "Footer".to_string(),
        SectionKind::Custom(name) => name.clone(),
    }
}
