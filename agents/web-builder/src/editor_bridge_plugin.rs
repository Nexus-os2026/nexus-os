//! Editor Bridge Plugin — injects the visual editor bridge script into Vite-served pages.
//!
//! For React projects, the EditorBridge script must be present in the Vite-served
//! index.html. The simplest approach: include the script directly in the generated
//! index.html file, controlled by a flag in the HTML template.
//!
//! This module also generates a small Vite plugin file that injects the bridge
//! script via `transformIndexHtml` — used when the developer wants the bridge
//! only in dev mode.

/// Generate the EditorBridge script as a standalone JS string.
///
/// This is the same script from EditorBridge.ts but compiled to plain JS
/// for injection into non-TypeScript contexts (Vite-served index.html).
pub fn editor_bridge_script_js() -> &'static str {
    // The bridge script — same logic as the TypeScript EditorBridge.getEditorBridgeScript()
    // but as a static string for Rust-side injection.
    r#"(function(){
'use strict';
var editMode=false,highlightEl=null,selectedEl=null;
var KEYS=['color','backgroundColor','borderColor','borderRadius','fontSize','fontFamily','fontWeight','padding','paddingTop','paddingBottom','paddingLeft','paddingRight','margin','marginTop','marginBottom'];
function getComputedProps(el){var cs=window.getComputedStyle(el);var r={};for(var i=0;i<KEYS.length;i++)r[KEYS[i]]=cs[KEYS[i]]||'';return r}
function getSection(el){var c=el;while(c&&c!==document.body){if(c.getAttribute&&c.getAttribute('data-nexus-section'))return c.getAttribute('data-nexus-section');c=c.parentElement}return null}
function getSlot(el){var c=el;while(c&&c!==document.body){if(c.getAttribute&&c.getAttribute('data-nexus-slot'))return c.getAttribute('data-nexus-slot');c=c.parentElement}return null}
function getRect(el){var r=el.getBoundingClientRect();return{top:r.top,left:r.left,width:r.width,height:r.height}}
function getTextContent(el){var t='';for(var i=0;i<el.childNodes.length;i++){if(el.childNodes[i].nodeType===3)t+=el.childNodes[i].textContent}return t.trim()||el.textContent.trim().substring(0,200)}
function ensureHighlight(){if(!highlightEl){highlightEl=document.createElement('div');highlightEl.id='__nexus-highlight';highlightEl.style.cssText='position:fixed;pointer-events:none;border:2px solid #00d4aa;border-radius:4px;background:rgba(0,212,170,0.06);z-index:999999;display:none;transition:all 80ms ease;box-shadow:0 0 0 1px rgba(0,212,170,0.15);';document.body.appendChild(highlightEl)}return highlightEl}
function showHighlight(r){var h=ensureHighlight();h.style.top=r.top+'px';h.style.left=r.left+'px';h.style.width=r.width+'px';h.style.height=r.height+'px';h.style.display='block'}
function hideHighlight(){if(highlightEl)highlightEl.style.display='none'}
function sendToParent(msg){window.parent.postMessage(msg,'*')}
function onMouseOver(e){if(!editMode)return;var el=e.target;if(el.id==='__nexus-highlight')return;var r=getRect(el);showHighlight(r);sendToParent({type:'element-hover',sectionId:getSection(el),slotName:getSlot(el),elementTag:el.tagName.toLowerCase(),rect:r,computedStyles:getComputedProps(el)})}
function onClick(e){if(!editMode)return;e.preventDefault();e.stopPropagation();var el=e.target;if(el.id==='__nexus-highlight')return;selectedEl=el;var r=getRect(el);showHighlight(r);sendToParent({type:'element-select',sectionId:getSection(el),slotName:getSlot(el),elementTag:el.tagName.toLowerCase(),rect:r,computedStyles:getComputedProps(el),currentText:getTextContent(el)})}
window.addEventListener('message',function(e){var d=e.data;if(!d||!d.type)return;switch(d.type){case'enable-edit-mode':editMode=true;document.body.style.cursor='crosshair';document.addEventListener('mouseover',onMouseOver,true);document.addEventListener('click',onClick,true);sendToParent({type:'edit-mode-ready'});break;case'disable-edit-mode':editMode=false;document.body.style.cursor='';document.removeEventListener('mouseover',onMouseOver,true);document.removeEventListener('click',onClick,true);hideHighlight();selectedEl=null;break;case'clear-highlight':hideHighlight();break;case'update-token':document.documentElement.style.setProperty('--'+d.tokenName,d.value);break;case'update-section-token':var s=document.querySelector('[data-nexus-section="'+d.sectionId+'"]');if(s)s.style.setProperty('--'+d.tokenName,d.value);break;case'highlight':if(d.rect)showHighlight(d.rect);break}})
})();"#
}

/// Generate an index.html that includes the editor bridge script.
///
/// This is used when generating the React project's index.html —
/// the bridge script is injected before `</body>` so it runs after
/// the React app mounts.
pub fn inject_bridge_into_html(html: &str) -> String {
    let script = format!("<script>{}</script>", editor_bridge_script_js());
    if let Some(pos) = html.rfind("</body>") {
        format!("{}{}{}", &html[..pos], script, &html[pos..])
    } else {
        format!("{html}\n{script}")
    }
}

/// Generate a Vite plugin file that injects the bridge script in dev mode.
///
/// This is an alternative to direct HTML injection — the plugin only
/// activates during `vite dev`, keeping the production build clean.
pub fn generate_vite_bridge_plugin() -> String {
    format!(
        r#"// Auto-generated by Nexus Builder — EditorBridge Vite plugin
// Injects the visual editor bridge script in development mode only.

const BRIDGE_SCRIPT = `{}`;

export default function nexusEditorBridge() {{
  return {{
    name: 'nexus-editor-bridge',
    transformIndexHtml(html: string) {{
      if (process.env.NODE_ENV === 'production') return html;
      return html.replace('</body>', `<script>${{BRIDGE_SCRIPT}}</script></body>`);
    }},
  }};
}}
"#,
        editor_bridge_script_js()
            .replace('`', "\\`")
            .replace("${", "\\${")
    )
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_script_is_valid_js() {
        let script = editor_bridge_script_js();
        // Basic syntax checks — properly balanced parens/braces
        let open_parens = script.matches('(').count();
        let close_parens = script.matches(')').count();
        assert_eq!(open_parens, close_parens, "unbalanced parentheses");

        let open_braces = script.matches('{').count();
        let close_braces = script.matches('}').count();
        assert_eq!(open_braces, close_braces, "unbalanced braces");

        // Should start with IIFE
        assert!(script.starts_with("(function()"));
        assert!(script.ends_with("})();"));
    }

    #[test]
    fn test_inject_bridge_into_html() {
        let html = "<html><body><div id=\"root\"></div></body></html>";
        let result = inject_bridge_into_html(html);
        assert!(result.contains("<script>"));
        assert!(result.contains("__nexus-highlight"));
        // Script should be before </body>
        let script_pos = result.find("<script>").unwrap();
        let body_pos = result.rfind("</body>").unwrap();
        assert!(script_pos < body_pos, "script should be before </body>");
    }

    #[test]
    fn test_vite_plugin_generates_valid_ts() {
        let plugin = generate_vite_bridge_plugin();
        assert!(plugin.contains("export default function nexusEditorBridge"));
        assert!(plugin.contains("transformIndexHtml"));
        assert!(plugin.contains("name: 'nexus-editor-bridge'"));
    }

    #[test]
    fn test_bridge_script_handles_all_message_types() {
        let script = editor_bridge_script_js();
        assert!(script.contains("enable-edit-mode"));
        assert!(script.contains("disable-edit-mode"));
        assert!(script.contains("clear-highlight"));
        assert!(script.contains("update-token"));
        assert!(script.contains("update-section-token"));
        assert!(script.contains("element-hover"));
        assert!(script.contains("element-select"));
    }
}
