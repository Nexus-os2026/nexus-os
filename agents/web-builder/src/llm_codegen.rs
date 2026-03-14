//! LLM-enhanced website generation: produces real, production-quality websites via an LLM provider.

use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_sdk::errors::AgentError;
use std::path::{Path, PathBuf};

/// Generate a complete single-page website using an LLM provider.
///
/// Sends a crafted system prompt to the LLM, parses out HTML/CSS/JS from the response,
/// and writes `index.html`, `styles.css`, and `script.js` to `output_dir`.
pub fn generate_site_with_llm<P: LlmProvider>(
    description: &str,
    output_dir: &Path,
    gateway: &mut GovernedLlmGateway<P>,
    context: &mut AgentRuntimeContext,
    model: &str,
) -> Result<Vec<PathBuf>, AgentError> {
    let prompt = build_prompt(description);

    let response = gateway.query(context, &prompt, 4000, model)?;
    let (html, css, js) = parse_code_blocks(&response.output_text);

    std::fs::create_dir_all(output_dir)
        .map_err(|e| AgentError::ManifestError(format!("failed to create output dir: {e}")))?;

    let files = [("index.html", html), ("styles.css", css), ("script.js", js)];

    let mut created = Vec::new();
    for (name, content) in &files {
        if content.is_empty() {
            continue;
        }
        let path = output_dir.join(name);
        std::fs::write(&path, content)
            .map_err(|e| AgentError::ManifestError(format!("failed to write {name}: {e}")))?;
        created.push(path);
    }

    Ok(created)
}

/// Build the system prompt for LLM website generation.
fn build_prompt(description: &str) -> String {
    let base = format!(
        "You are an elite web developer. Generate a complete, production-quality single-page \
         website based on the description. Return EXACTLY three code blocks:\n\n\
         ```html\n(complete HTML with proper head, meta tags, links to styles.css and script.js)\n```\n\
         ```css\n(complete CSS with modern design, responsive layout, smooth animations, dark mode support if requested)\n```\n\
         ```javascript\n(complete JS with interactivity, smooth scrolling, any requested functionality)\n```\n\n\
         Make it visually stunning. Use modern CSS (grid, flexbox, custom properties). \
         Add subtle animations. Make it responsive.\n\n\
         Description: {description}"
    );

    let lower = description.to_lowercase();
    let needs_3d = [
        "3d",
        "three.js",
        "animated hero",
        "particles",
        "webgl",
        "globe",
    ]
    .iter()
    .any(|kw| lower.contains(kw));

    if needs_3d {
        format!(
            "{base}\n\n\
             Also include a Three.js 3D scene. Import from: \
             https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js. \
             Create an impressive animated 3D hero section (rotating geometry, particle system, \
             or animated wave). Use requestAnimationFrame."
        )
    } else {
        base
    }
}

/// Parse HTML, CSS, and JS from fenced code blocks in the LLM response.
///
/// Looks for blocks fenced with ```html, ```css, and ```javascript or ```js.
/// If no fences are found at all, the entire response is treated as HTML.
pub fn parse_code_blocks(response: &str) -> (String, String, String) {
    let html = extract_block(response, &["html"]).unwrap_or_else(|| {
        // Fallback: if no fences found at all, treat entire response as HTML
        if extract_block(response, &["css"]).is_none()
            && extract_block(response, &["javascript", "js"]).is_none()
        {
            response.trim().to_string()
        } else {
            String::new()
        }
    });
    let css = extract_block(response, &["css"]).unwrap_or_default();
    let js = extract_block(response, &["javascript", "js"]).unwrap_or_default();

    (html, css, js)
}

/// Extract content from a fenced code block matching any of the given language labels.
fn extract_block(text: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        let marker = format!("```{label}");
        if let Some(start_idx) = text.find(&marker) {
            let after_marker = start_idx + marker.len();
            let rest = &text[after_marker..];
            // Skip to next newline (past any trailing text on the opener line)
            if let Some(nl) = rest.find('\n') {
                let content_start = &rest[nl + 1..];
                if let Some(end) = content_start.find("```") {
                    let block = content_start[..end].trim().to_string();
                    if !block.is_empty() {
                        return Some(block);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_code_blocks_properly_fenced() {
        let response = r#"Here is your website:

```html
<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body><h1>Hello</h1></body>
</html>
```

```css
body { margin: 0; font-family: sans-serif; }
h1 { color: blue; }
```

```javascript
document.addEventListener('DOMContentLoaded', () => {
    console.log('ready');
});
```
"#;
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(css.contains("font-family: sans-serif"));
        assert!(js.contains("DOMContentLoaded"));
    }

    #[test]
    fn test_parse_code_blocks_js_label() {
        let response = r#"```html
<html><body>Hi</body></html>
```

```css
body { color: red; }
```

```js
alert('hi');
```
"#;
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("<body>Hi</body>"));
        assert!(css.contains("color: red"));
        assert!(js.contains("alert"));
    }

    #[test]
    fn test_parse_code_blocks_no_fences_fallback() {
        let response = "<html><body>Fallback content</body></html>";
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("Fallback content"));
        assert!(css.is_empty());
        assert!(js.is_empty());
    }

    #[test]
    fn test_parse_code_blocks_partial_fences() {
        let response = r#"```html
<div>Only HTML here</div>
```

No CSS or JS blocks provided.
"#;
        let (html, css, js) = parse_code_blocks(response);
        assert!(html.contains("<div>Only HTML here</div>"));
        assert!(css.is_empty());
        assert!(js.is_empty());
    }

    #[test]
    fn test_parse_code_blocks_empty_blocks() {
        let response = r#"```html
```

```css
```

```javascript
```
"#;
        let (_html, css, js) = parse_code_blocks(response);
        // All blocks are empty — HTML fallback kicks in since no real content was found
        assert!(css.is_empty());
        assert!(js.is_empty());
    }

    #[test]
    fn test_build_prompt_basic() {
        let prompt = build_prompt("a portfolio site");
        assert!(prompt.contains("elite web developer"));
        assert!(prompt.contains("a portfolio site"));
        assert!(!prompt.contains("Three.js"));
    }

    #[test]
    fn test_build_prompt_3d() {
        let prompt = build_prompt("a landing page with 3D globe");
        assert!(prompt.contains("Three.js"));
        assert!(prompt.contains("requestAnimationFrame"));
    }

    #[test]
    fn test_build_prompt_particles() {
        let prompt = build_prompt("site with particles background");
        assert!(prompt.contains("Three.js"));
    }
}
