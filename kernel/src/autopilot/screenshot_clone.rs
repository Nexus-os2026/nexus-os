//! Screenshot Clone — vision analysis of app screenshots into deployable project specs.

use serde::{Deserialize, Serialize};

/// A UI component extracted from a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiComponent {
    pub component_type: String,
    pub label: Option<String>,
    pub position: ComponentPosition,
    pub style: ComponentStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPosition {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStyle {
    pub background_color: Option<String>,
    pub text_color: Option<String>,
    pub font_size: Option<f64>,
    pub border_radius: Option<f64>,
}

/// Layout structure detected from a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutStructure {
    pub layout_type: LayoutType,
    pub has_sidebar: bool,
    pub has_topnav: bool,
    pub has_footer: bool,
    pub grid_columns: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutType {
    Dashboard,
    Landing,
    Form,
    List,
    Detail,
    Grid,
    Custom,
}

/// Color palette extracted from the screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette {
    pub primary: String,
    pub secondary: String,
    pub background: String,
    pub text: String,
    pub accent: Vec<String>,
}

/// Data model inferred from visible UI elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredEntity {
    pub name: String,
    pub fields: Vec<String>,
    pub relationships: Vec<String>,
}

/// Features detected from the screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedFeature {
    pub name: String,
    pub category: FeatureCategory,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureCategory {
    Auth,
    Search,
    Crud,
    Payments,
    Messaging,
    Analytics,
    FileUpload,
    Navigation,
    Other,
}

/// Complete analysis of a screenshot, ready for project generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotAnalysis {
    pub components: Vec<UiComponent>,
    pub layout: LayoutStructure,
    pub colors: ColorPalette,
    pub entities: Vec<InferredEntity>,
    pub features: Vec<DetectedFeature>,
    pub responsive: bool,
    pub app_type: String,
}

/// Project specification generated from a screenshot analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSpec {
    pub name: String,
    pub app_type: String,
    pub frontend: FrontendSpec,
    pub backend: BackendSpec,
    pub database: DatabaseSpec,
    pub visual_match_target: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendSpec {
    pub framework: String,
    pub components: Vec<String>,
    pub css_framework: String,
    pub pages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSpec {
    pub framework: String,
    pub endpoints: Vec<EndpointSpec>,
    pub auth_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSpec {
    pub method: String,
    pub path: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSpec {
    pub db_type: String,
    pub tables: Vec<TableSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSpec {
    pub name: String,
    pub columns: Vec<String>,
}

/// Visual diff result comparing the clone against the original screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualDiffResult {
    pub match_score: f64,
    pub mismatched_regions: Vec<String>,
    pub passed: bool,
}

/// The screenshot-to-clone engine.
#[derive(Debug, Clone)]
pub struct ScreenshotCloner {
    pub min_visual_match: f64,
}

impl Default for ScreenshotCloner {
    fn default() -> Self {
        Self {
            min_visual_match: 0.85,
        }
    }
}

impl ScreenshotCloner {
    pub fn new(min_visual_match: f64) -> Self {
        Self { min_visual_match }
    }

    /// Build the vision prompt for analyzing a screenshot.
    pub fn build_analysis_prompt(&self, image_path: &str) -> (String, String) {
        let system = "You are a UI analysis engine. Given an app screenshot, extract every detail."
            .to_string();
        let user = format!(
            "Analyze the app screenshot at '{}'. Extract:\n\
             1. Every UI component (buttons, forms, cards, navigation, modals)\n\
             2. Layout structure (sidebar? topnav? grid? list?)\n\
             3. Color palette (exact hex values)\n\
             4. Typography (font sizes, weights)\n\
             5. Data model implied by the UI (what entities exist?)\n\
             6. Features visible (auth, search, CRUD, payments, messaging?)\n\
             7. Responsive hints (mobile? desktop? both?)\n\
             Return as detailed JSON matching the ScreenshotAnalysis schema.",
            image_path
        );
        (system, user)
    }

    /// Parse an LLM response into a ScreenshotAnalysis.
    pub fn parse_analysis(&self, response: &str) -> Result<ScreenshotAnalysis, String> {
        serde_json::from_str(response).map_err(|e| format!("Failed to parse analysis: {e}"))
    }

    /// Generate a project spec from a screenshot analysis.
    pub fn generate_project_spec(
        &self,
        analysis: &ScreenshotAnalysis,
        project_name: &str,
    ) -> ProjectSpec {
        let frontend_components: Vec<String> = analysis
            .components
            .iter()
            .map(|c| c.component_type.clone())
            .collect();

        let endpoints: Vec<EndpointSpec> = analysis
            .entities
            .iter()
            .flat_map(|entity| {
                let name = entity.name.to_lowercase();
                vec![
                    EndpointSpec {
                        method: "GET".into(),
                        path: format!("/api/{name}"),
                        description: format!("List all {name}"),
                    },
                    EndpointSpec {
                        method: "POST".into(),
                        path: format!("/api/{name}"),
                        description: format!("Create {name}"),
                    },
                    EndpointSpec {
                        method: "GET".into(),
                        path: format!("/api/{name}/{{id}}"),
                        description: format!("Get {name} by ID"),
                    },
                    EndpointSpec {
                        method: "PUT".into(),
                        path: format!("/api/{name}/{{id}}"),
                        description: format!("Update {name}"),
                    },
                    EndpointSpec {
                        method: "DELETE".into(),
                        path: format!("/api/{name}/{{id}}"),
                        description: format!("Delete {name}"),
                    },
                ]
            })
            .collect();

        let tables: Vec<TableSpec> = analysis
            .entities
            .iter()
            .map(|entity| TableSpec {
                name: entity.name.to_lowercase(),
                columns: entity.fields.clone(),
            })
            .collect();

        let has_auth = analysis
            .features
            .iter()
            .any(|f| matches!(f.category, FeatureCategory::Auth));

        ProjectSpec {
            name: project_name.to_string(),
            app_type: analysis.app_type.clone(),
            frontend: FrontendSpec {
                framework: "React".into(),
                components: frontend_components,
                css_framework: "Tailwind".into(),
                pages: vec!["Home".into(), "Dashboard".into()],
            },
            backend: BackendSpec {
                framework: "FastAPI".into(),
                endpoints,
                auth_type: if has_auth { Some("JWT".into()) } else { None },
            },
            database: DatabaseSpec {
                db_type: "PostgreSQL".into(),
                tables,
            },
            visual_match_target: self.min_visual_match,
        }
    }

    /// Check if a visual diff result passes the minimum threshold.
    pub fn check_visual_match(&self, diff: &VisualDiffResult) -> bool {
        diff.match_score >= self.min_visual_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screenshot_cloner_default() {
        let cloner = ScreenshotCloner::default();
        assert!((cloner.min_visual_match - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_build_analysis_prompt() {
        let cloner = ScreenshotCloner::default();
        let (system, user) = cloner.build_analysis_prompt("/tmp/screenshot.png");
        assert!(system.contains("UI analysis"));
        assert!(user.contains("/tmp/screenshot.png"));
        assert!(user.contains("Color palette"));
    }

    #[test]
    fn test_generate_project_spec() {
        let cloner = ScreenshotCloner::default();
        let analysis = ScreenshotAnalysis {
            components: vec![UiComponent {
                component_type: "Button".into(),
                label: Some("Sign In".into()),
                position: ComponentPosition {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 40.0,
                },
                style: ComponentStyle {
                    background_color: Some("#635BFF".into()),
                    text_color: Some("#FFFFFF".into()),
                    font_size: Some(14.0),
                    border_radius: Some(4.0),
                },
            }],
            layout: LayoutStructure {
                layout_type: LayoutType::Dashboard,
                has_sidebar: true,
                has_topnav: true,
                has_footer: false,
                grid_columns: Some(3),
            },
            colors: ColorPalette {
                primary: "#635BFF".into(),
                secondary: "#1A1F36".into(),
                background: "#F6F9FC".into(),
                text: "#1A1F36".into(),
                accent: vec!["#00D4AA".into()],
            },
            entities: vec![InferredEntity {
                name: "Transaction".into(),
                fields: vec!["id".into(), "amount".into(), "status".into()],
                relationships: vec!["belongs_to Customer".into()],
            }],
            features: vec![DetectedFeature {
                name: "Authentication".into(),
                category: FeatureCategory::Auth,
                confidence: 0.95,
            }],
            responsive: true,
            app_type: "Dashboard".into(),
        };

        let spec = cloner.generate_project_spec(&analysis, "stripe-clone");
        assert_eq!(spec.name, "stripe-clone");
        assert_eq!(spec.backend.endpoints.len(), 5); // CRUD for Transaction
        assert_eq!(spec.backend.auth_type, Some("JWT".into()));
        assert_eq!(spec.database.tables[0].name, "transaction");
    }

    #[test]
    fn test_visual_match_check() {
        let cloner = ScreenshotCloner::new(0.90);
        let pass = VisualDiffResult {
            match_score: 0.92,
            mismatched_regions: vec![],
            passed: true,
        };
        let fail = VisualDiffResult {
            match_score: 0.80,
            mismatched_regions: vec!["header".into()],
            passed: false,
        };
        assert!(cloner.check_visual_match(&pass));
        assert!(!cloner.check_visual_match(&fail));
    }

    #[test]
    fn test_parse_analysis_valid() {
        let cloner = ScreenshotCloner::default();
        let json = serde_json::json!({
            "components": [],
            "layout": {
                "layout_type": "Dashboard",
                "has_sidebar": true,
                "has_topnav": false,
                "has_footer": false,
                "grid_columns": null
            },
            "colors": {
                "primary": "#000",
                "secondary": "#111",
                "background": "#fff",
                "text": "#000",
                "accent": []
            },
            "entities": [],
            "features": [],
            "responsive": true,
            "app_type": "SaaS"
        });
        let result = cloner.parse_analysis(&json.to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().app_type, "SaaS");
    }

    #[test]
    fn test_parse_analysis_invalid() {
        let cloner = ScreenshotCloner::default();
        let result = cloner.parse_analysis("not json");
        assert!(result.is_err());
    }
}
