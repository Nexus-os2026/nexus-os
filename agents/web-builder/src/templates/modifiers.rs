//! Template modifiers: composable HTML snippets injected into any template.
//!
//! Each modifier is a self-contained `<section>` with `data-nexus-section` anchors.
//! Modifiers are injected BEFORE the `<footer>` of the template.

/// A composable HTML snippet that can be injected into any template.
pub struct TemplateModifier {
    pub id: &'static str,
    pub name: &'static str,
    pub html_snippet: &'static str,
}

static DOCS_SIDEBAR: TemplateModifier = TemplateModifier {
    id: "docs_sidebar",
    name: "Documentation Sidebar",
    html_snippet: r##"
  <!-- Modifier: Documentation Sidebar -->
  <section data-nexus-section="mod_docs_sidebar" data-nexus-editable="true" style="position: fixed; left: 0; top: 60px; width: 260px; height: calc(100vh - 60px); overflow-y: auto; padding: 1.5rem 1rem; background: #f8fafc; border-right: 1px solid #e2e8f0; z-index: 40;">
    <nav aria-label="Documentation sidebar navigation">
      <h3 style="font-size: 0.75rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: #64748b; margin-bottom: 0.75rem;" data-nexus-slot="sidebar_title">{{SIDEBAR_TITLE}}</h3>
      <ul style="list-style: none; padding: 0; margin: 0;" data-nexus-slot="sidebar_links">
        <li><a href="#" style="display: block; padding: 0.4rem 0.75rem; font-size: 0.875rem; color: #64748b; border-radius: 0.375rem; text-decoration: none;" aria-label="Section 1">{{SIDEBAR_LINK_1}}</a></li>
        <li><a href="#" style="display: block; padding: 0.4rem 0.75rem; font-size: 0.875rem; color: #64748b; border-radius: 0.375rem; text-decoration: none;" aria-label="Section 2">{{SIDEBAR_LINK_2}}</a></li>
        <li><a href="#" style="display: block; padding: 0.4rem 0.75rem; font-size: 0.875rem; color: #64748b; border-radius: 0.375rem; text-decoration: none;" aria-label="Section 3">{{SIDEBAR_LINK_3}}</a></li>
      </ul>
    </nav>
  </section>
"##,
};

static PRICING_CALCULATOR: TemplateModifier = TemplateModifier {
    id: "pricing_calculator",
    name: "Interactive Pricing Calculator",
    html_snippet: r##"
  <!-- Modifier: Pricing Calculator -->
  <section data-nexus-section="mod_pricing_calculator" data-nexus-editable="true" style="padding: 4rem 2rem; max-width: 36rem; margin: 0 auto; text-align: center;">
    <h2 style="font-size: 1.75rem; font-weight: 700; margin-bottom: 1.5rem;" data-nexus-slot="calculator_heading">{{CALCULATOR_HEADING}}</h2>
    <div style="background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); border-radius: 0.75rem; padding: 2rem;">
      <label style="display: block; font-size: 0.9rem; color: #94a3b8; margin-bottom: 0.5rem;" for="calc-slider" aria-label="Adjust quantity">{{SLIDER_LABEL}}</label>
      <input type="range" id="calc-slider" min="1" max="100" value="10" style="width: 100%; margin-bottom: 1rem;" aria-label="Quantity slider" />
      <div style="font-size: 2rem; font-weight: 800; margin-bottom: 0.5rem;" data-nexus-slot="calculator_price">{{CALCULATED_PRICE}}</div>
      <p style="color: #94a3b8; font-size: 0.85rem;" data-nexus-slot="calculator_note">{{CALCULATOR_NOTE}}</p>
      <button style="margin-top: 1.5rem; padding: 0.75rem 2rem; background: #6366f1; color: #fff; border: none; border-radius: 0.5rem; font-size: 1rem; cursor: pointer;" aria-label="Get started with selected plan">Get Started</button>
    </div>
  </section>
"##,
};

static BOOKING_FORM: TemplateModifier = TemplateModifier {
    id: "booking_form",
    name: "Reservation / Booking Form",
    html_snippet: r##"
  <!-- Modifier: Booking Form -->
  <section data-nexus-section="mod_booking_form" data-nexus-editable="true" style="padding: 4rem 2rem; max-width: 32rem; margin: 0 auto;">
    <h2 style="font-size: 1.75rem; font-weight: 700; text-align: center; margin-bottom: 2rem;" data-nexus-slot="booking_heading">{{BOOKING_HEADING}}</h2>
    <form aria-label="Reservation form" style="display: flex; flex-direction: column; gap: 1rem;">
      <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem;">
        <div>
          <label style="display: block; font-size: 0.85rem; font-weight: 500; margin-bottom: 0.25rem;" for="booking-date">Date</label>
          <input type="date" id="booking-date" style="width: 100%; padding: 0.6rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem;" aria-label="Reservation date" required />
        </div>
        <div>
          <label style="display: block; font-size: 0.85rem; font-weight: 500; margin-bottom: 0.25rem;" for="booking-time">Time</label>
          <input type="time" id="booking-time" style="width: 100%; padding: 0.6rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem;" aria-label="Reservation time" required />
        </div>
      </div>
      <div>
        <label style="display: block; font-size: 0.85rem; font-weight: 500; margin-bottom: 0.25rem;" for="booking-party">Party Size</label>
        <select id="booking-party" style="width: 100%; padding: 0.6rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem;" aria-label="Party size">
          <option>1 Guest</option>
          <option>2 Guests</option>
          <option>3-4 Guests</option>
          <option>5-6 Guests</option>
          <option>7+ Guests</option>
        </select>
      </div>
      <div>
        <label style="display: block; font-size: 0.85rem; font-weight: 500; margin-bottom: 0.25rem;" for="booking-name">Name</label>
        <input type="text" id="booking-name" placeholder="Your name" style="width: 100%; padding: 0.6rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem;" aria-label="Your name" required />
      </div>
      <div>
        <label style="display: block; font-size: 0.85rem; font-weight: 500; margin-bottom: 0.25rem;" for="booking-phone">Phone</label>
        <input type="tel" id="booking-phone" placeholder="Your phone number" style="width: 100%; padding: 0.6rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem;" aria-label="Your phone number" required />
      </div>
      <button type="submit" style="padding: 0.75rem; background: #d97706; color: #fff; border: none; border-radius: 0.5rem; font-size: 1rem; font-weight: 600; cursor: pointer;" aria-label="Submit reservation">Book Now</button>
    </form>
  </section>
"##,
};

static PHOTO_GALLERY: TemplateModifier = TemplateModifier {
    id: "photo_gallery",
    name: "Photo Gallery Grid",
    html_snippet: r##"
  <!-- Modifier: Photo Gallery -->
  <section data-nexus-section="mod_photo_gallery" data-nexus-editable="true" style="padding: 4rem 2rem; max-width: 72rem; margin: 0 auto;">
    <h2 style="font-size: 1.75rem; font-weight: 700; text-align: center; margin-bottom: 2rem;" data-nexus-slot="gallery_heading">{{GALLERY_HEADING}}</h2>
    <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.75rem;" data-nexus-slot="gallery_items">
      <div style="aspect-ratio: 4/3; background: linear-gradient(135deg, #e0e7ff, #fce7f3); border-radius: 0.5rem; display: flex; align-items: center; justify-content: center; color: #6b7280; font-size: 0.85rem; cursor: pointer; transition: transform 0.3s ease;" role="img" aria-label="Gallery image 1">{{GALLERY_IMG_1}}</div>
      <div style="aspect-ratio: 4/3; background: linear-gradient(135deg, #fef3c7, #fde68a); border-radius: 0.5rem; display: flex; align-items: center; justify-content: center; color: #6b7280; font-size: 0.85rem; cursor: pointer; transition: transform 0.3s ease;" role="img" aria-label="Gallery image 2">{{GALLERY_IMG_2}}</div>
      <div style="aspect-ratio: 4/3; background: linear-gradient(135deg, #d1fae5, #a7f3d0); border-radius: 0.5rem; display: flex; align-items: center; justify-content: center; color: #6b7280; font-size: 0.85rem; cursor: pointer; transition: transform 0.3s ease;" role="img" aria-label="Gallery image 3">{{GALLERY_IMG_3}}</div>
      <div style="aspect-ratio: 4/3; background: linear-gradient(135deg, #fce7f3, #fbcfe8); border-radius: 0.5rem; display: flex; align-items: center; justify-content: center; color: #6b7280; font-size: 0.85rem; cursor: pointer; transition: transform 0.3s ease;" role="img" aria-label="Gallery image 4">{{GALLERY_IMG_4}}</div>
      <div style="aspect-ratio: 4/3; background: linear-gradient(135deg, #e0e7ff, #c7d2fe); border-radius: 0.5rem; display: flex; align-items: center; justify-content: center; color: #6b7280; font-size: 0.85rem; cursor: pointer; transition: transform 0.3s ease;" role="img" aria-label="Gallery image 5">{{GALLERY_IMG_5}}</div>
      <div style="aspect-ratio: 4/3; background: linear-gradient(135deg, #fef9c3, #fde68a); border-radius: 0.5rem; display: flex; align-items: center; justify-content: center; color: #6b7280; font-size: 0.85rem; cursor: pointer; transition: transform 0.3s ease;" role="img" aria-label="Gallery image 6">{{GALLERY_IMG_6}}</div>
    </div>
  </section>
"##,
};

static CONTACT_MAP: TemplateModifier = TemplateModifier {
    id: "contact_map",
    name: "Contact + Map",
    html_snippet: r##"
  <!-- Modifier: Contact + Map -->
  <section data-nexus-section="mod_contact_map" data-nexus-editable="true" style="padding: 4rem 2rem; max-width: 72rem; margin: 0 auto; display: grid; grid-template-columns: 1fr 1fr; gap: 2rem;">
    <div style="background: #e5e7eb; border-radius: 0.75rem; min-height: 320px; display: flex; align-items: center; justify-content: center; color: #6b7280; font-size: 0.9rem;" role="img" aria-label="Map showing location" data-nexus-slot="map_placeholder">{{MAP_PLACEHOLDER}}</div>
    <div>
      <h2 style="font-size: 1.5rem; font-weight: 700; margin-bottom: 1.5rem;" data-nexus-slot="contact_heading">{{CONTACT_HEADING}}</h2>
      <form aria-label="Contact form" style="display: flex; flex-direction: column; gap: 1rem;">
        <input type="text" placeholder="Your Name" style="padding: 0.6rem 1rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem;" aria-label="Your name" required />
        <input type="email" placeholder="Your Email" style="padding: 0.6rem 1rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem;" aria-label="Your email" required />
        <textarea placeholder="Your Message" style="padding: 0.6rem 1rem; border: 1px solid #e5e7eb; border-radius: 0.5rem; font-size: 0.9rem; min-height: 100px; resize: vertical;" aria-label="Your message" required></textarea>
        <button type="submit" style="padding: 0.75rem; background: #111827; color: #fff; border: none; border-radius: 0.5rem; font-size: 0.95rem; cursor: pointer;" aria-label="Send message">Send Message</button>
      </form>
    </div>
  </section>
"##,
};

static BLOG_FEED: TemplateModifier = TemplateModifier {
    id: "blog_feed",
    name: "Blog / Recent Posts Feed",
    html_snippet: r##"
  <!-- Modifier: Blog Feed -->
  <section data-nexus-section="mod_blog_feed" data-nexus-editable="true" style="padding: 4rem 2rem; max-width: 72rem; margin: 0 auto;">
    <h2 style="font-size: 1.75rem; font-weight: 700; text-align: center; margin-bottom: 2.5rem;" data-nexus-slot="blog_heading">{{BLOG_HEADING}}</h2>
    <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 1.5rem;" data-nexus-slot="blog_posts">
      <article style="background: #fff; border: 1px solid #e5e7eb; border-radius: 0.75rem; overflow: hidden; transition: box-shadow 0.3s ease;">
        <div style="width: 100%; aspect-ratio: 16/9; background: linear-gradient(135deg, #e0e7ff, #fce7f3);" role="img" aria-label="Blog post image"></div>
        <div style="padding: 1.25rem;">
          <span style="font-size: 0.75rem; color: #6b7280;">{{POST_1_DATE}}</span>
          <h3 style="font-size: 1rem; font-weight: 600; margin: 0.5rem 0;">{{POST_1_TITLE}}</h3>
          <p style="font-size: 0.85rem; color: #6b7280; line-height: 1.5;">{{POST_1_EXCERPT}}</p>
          <a href="#" style="display: inline-block; margin-top: 0.75rem; font-size: 0.85rem; font-weight: 500; color: #6366f1; text-decoration: none;" aria-label="Read full post">Read More →</a>
        </div>
      </article>
      <article style="background: #fff; border: 1px solid #e5e7eb; border-radius: 0.75rem; overflow: hidden; transition: box-shadow 0.3s ease;">
        <div style="width: 100%; aspect-ratio: 16/9; background: linear-gradient(135deg, #fef3c7, #fde68a);" role="img" aria-label="Blog post image"></div>
        <div style="padding: 1.25rem;">
          <span style="font-size: 0.75rem; color: #6b7280;">{{POST_2_DATE}}</span>
          <h3 style="font-size: 1rem; font-weight: 600; margin: 0.5rem 0;">{{POST_2_TITLE}}</h3>
          <p style="font-size: 0.85rem; color: #6b7280; line-height: 1.5;">{{POST_2_EXCERPT}}</p>
          <a href="#" style="display: inline-block; margin-top: 0.75rem; font-size: 0.85rem; font-weight: 500; color: #6366f1; text-decoration: none;" aria-label="Read full post">Read More →</a>
        </div>
      </article>
      <article style="background: #fff; border: 1px solid #e5e7eb; border-radius: 0.75rem; overflow: hidden; transition: box-shadow 0.3s ease;">
        <div style="width: 100%; aspect-ratio: 16/9; background: linear-gradient(135deg, #d1fae5, #a7f3d0);" role="img" aria-label="Blog post image"></div>
        <div style="padding: 1.25rem;">
          <span style="font-size: 0.75rem; color: #6b7280;">{{POST_3_DATE}}</span>
          <h3 style="font-size: 1rem; font-weight: 600; margin: 0.5rem 0;">{{POST_3_TITLE}}</h3>
          <p style="font-size: 0.85rem; color: #6b7280; line-height: 1.5;">{{POST_3_EXCERPT}}</p>
          <a href="#" style="display: inline-block; margin-top: 0.75rem; font-size: 0.85rem; font-weight: 500; color: #6366f1; text-decoration: none;" aria-label="Read full post">Read More →</a>
        </div>
      </article>
    </div>
  </section>
"##,
};

static ALL_MODIFIERS: &[&TemplateModifier] = &[
    &DOCS_SIDEBAR,
    &PRICING_CALCULATOR,
    &BOOKING_FORM,
    &PHOTO_GALLERY,
    &CONTACT_MAP,
    &BLOG_FEED,
];

/// Get a modifier by ID.
pub fn get_modifier(id: &str) -> Option<&'static TemplateModifier> {
    ALL_MODIFIERS.iter().find(|m| m.id == id).copied()
}

/// Apply modifiers by injecting their HTML snippets before the `<footer` tag.
/// If no `<footer` tag exists, snippets are appended before `</body>`.
pub fn apply_modifiers(template_html: &str, modifier_ids: &[String]) -> String {
    if modifier_ids.is_empty() {
        return template_html.to_string();
    }

    let mut snippets = String::new();
    for id in modifier_ids {
        if let Some(modifier) = get_modifier(id) {
            snippets.push_str(modifier.html_snippet);
            snippets.push('\n');
        }
    }

    if snippets.is_empty() {
        return template_html.to_string();
    }

    // Try to inject before <footer, falling back to </body>
    let lower = template_html.to_lowercase();
    if let Some(pos) = lower.rfind("<footer") {
        let (before, after) = template_html.split_at(pos);
        format!("{before}\n{snippets}\n  {after}")
    } else if let Some(pos) = lower.rfind("</body>") {
        let (before, after) = template_html.split_at(pos);
        format!("{before}\n{snippets}\n{after}")
    } else {
        format!("{template_html}\n{snippets}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_modifiers_count() {
        assert_eq!(ALL_MODIFIERS.len(), 6);
    }

    #[test]
    fn test_get_modifier_found() {
        let m = get_modifier("booking_form").unwrap();
        assert_eq!(m.name, "Reservation / Booking Form");
        assert!(m.html_snippet.contains("data-nexus-section"));
    }

    #[test]
    fn test_get_modifier_not_found() {
        assert!(get_modifier("nonexistent").is_none());
    }

    #[test]
    fn test_apply_modifiers_empty() {
        let html = "<html><body><footer>F</footer></body></html>";
        let result = apply_modifiers(html, &[]);
        assert_eq!(result, html);
    }

    #[test]
    fn test_apply_modifiers_before_footer() {
        let html = "<html><body><main>Content</main><footer>F</footer></body></html>";
        let result = apply_modifiers(html, &["blog_feed".to_string()]);
        assert!(result.contains("mod_blog_feed"));
        // Blog feed should appear before footer
        let blog_pos = result.find("mod_blog_feed").unwrap();
        let footer_pos = result.find("<footer>F</footer>").unwrap();
        assert!(blog_pos < footer_pos);
    }

    #[test]
    fn test_apply_modifiers_multiple() {
        let html = "<html><body><main>M</main><footer>F</footer></body></html>";
        let result = apply_modifiers(
            html,
            &["booking_form".to_string(), "photo_gallery".to_string()],
        );
        assert!(result.contains("mod_booking_form"));
        assert!(result.contains("mod_photo_gallery"));
    }

    #[test]
    fn test_all_modifiers_have_aria() {
        for m in ALL_MODIFIERS {
            assert!(
                m.html_snippet.contains("aria-label"),
                "Modifier {} missing aria-label",
                m.id
            );
        }
    }

    #[test]
    fn test_all_modifiers_have_nexus_section() {
        for m in ALL_MODIFIERS {
            assert!(
                m.html_snippet.contains("data-nexus-section"),
                "Modifier {} missing data-nexus-section",
                m.id
            );
        }
    }
}
