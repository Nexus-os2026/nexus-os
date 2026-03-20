//! Stress Test Simulator — generates simulated user personas and action sequences
//! for load-testing applications before deployment.

use serde::{Deserialize, Serialize};

/// A simulated user persona for stress testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPersona {
    pub name: String,
    pub behavior_type: BehaviorType,
    pub description: String,
    pub patience_level: f64,
    pub tech_savviness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BehaviorType {
    PowerUser,
    Novice,
    Adversarial,
    Mobile,
    SlowConnection,
    Impatient,
    DataHeavy,
}

/// A single user action in a stress test sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAction {
    pub action_type: ActionType,
    pub target: String,
    pub payload: Option<String>,
    pub delay_ms: u64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    Navigate,
    Click,
    Type,
    Submit,
    Upload,
    Scroll,
    Refresh,
    BackNavigation,
    RapidClick,
    InvalidInput,
    LargePayload,
    ConcurrentRequest,
}

/// An action sequence for a single simulated user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSequence {
    pub persona: UserPersona,
    pub actions: Vec<UserAction>,
}

/// A failure detected during stress simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressFailure {
    pub persona_name: String,
    pub action_index: usize,
    pub action_description: String,
    pub failure_type: FailureType,
    pub error_message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FailureType {
    Crash,
    Timeout,
    ErrorResponse,
    MemoryLeak,
    RaceCondition,
    DataCorruption,
    PerformanceDegradation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// Results from a complete stress test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressReport {
    pub total_personas: u32,
    pub total_actions: u32,
    pub actions_completed: u32,
    pub failures: Vec<StressFailure>,
    pub performance_metrics: PerformanceMetrics,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub avg_response_ms: f64,
    pub p95_response_ms: f64,
    pub p99_response_ms: f64,
    pub max_response_ms: f64,
    pub error_rate: f64,
    pub total_requests: u64,
}

/// The stress simulation engine.
#[derive(Debug, Clone)]
pub struct StressSimulator {
    pub num_users: u32,
    pub max_actions_per_user: u32,
    pub fail_on_critical: bool,
}

impl Default for StressSimulator {
    fn default() -> Self {
        Self {
            num_users: 100,
            max_actions_per_user: 20,
            fail_on_critical: true,
        }
    }
}

impl StressSimulator {
    pub fn new(num_users: u32) -> Self {
        Self {
            num_users,
            ..Default::default()
        }
    }

    /// Generate default user personas covering common behavior types.
    pub fn generate_default_personas(&self, count: u32) -> Vec<UserPersona> {
        let templates = [
            UserPersona {
                name: "Power User Pat".into(),
                behavior_type: BehaviorType::PowerUser,
                description:
                    "Uses keyboard shortcuts, opens multiple tabs, performs complex workflows"
                        .into(),
                patience_level: 0.9,
                tech_savviness: 0.95,
            },
            UserPersona {
                name: "Novice Nancy".into(),
                behavior_type: BehaviorType::Novice,
                description: "Clicks slowly, reads everything, confused by complex UIs".into(),
                patience_level: 0.7,
                tech_savviness: 0.2,
            },
            UserPersona {
                name: "Adversarial Alex".into(),
                behavior_type: BehaviorType::Adversarial,
                description:
                    "Enters invalid data, SQL injection attempts, XSS payloads, oversized files"
                        .into(),
                patience_level: 1.0,
                tech_savviness: 0.99,
            },
            UserPersona {
                name: "Mobile Morgan".into(),
                behavior_type: BehaviorType::Mobile,
                description: "Small screen, touch interactions, intermittent connection".into(),
                patience_level: 0.4,
                tech_savviness: 0.5,
            },
            UserPersona {
                name: "Slow Sam".into(),
                behavior_type: BehaviorType::SlowConnection,
                description: "2G connection, high latency, frequent timeouts".into(),
                patience_level: 0.6,
                tech_savviness: 0.3,
            },
            UserPersona {
                name: "Impatient Irene".into(),
                behavior_type: BehaviorType::Impatient,
                description:
                    "Clicks everything rapidly, doesn't wait for pages to load, rage-clicks".into(),
                patience_level: 0.1,
                tech_savviness: 0.6,
            },
            UserPersona {
                name: "Data-Heavy Dave".into(),
                behavior_type: BehaviorType::DataHeavy,
                description:
                    "Uploads large files, creates thousands of records, exports everything".into(),
                patience_level: 0.8,
                tech_savviness: 0.7,
            },
        ];

        let mut personas = Vec::new();
        for i in 0..count {
            let template = &templates[i as usize % templates.len()];
            let mut persona = template.clone();
            if i >= templates.len() as u32 {
                persona.name = format!("{} #{}", template.name, i);
            }
            personas.push(persona);
        }
        personas
    }

    /// Generate a realistic action sequence for a given persona.
    pub fn generate_actions_for_persona(&self, persona: &UserPersona) -> Vec<UserAction> {
        match persona.behavior_type {
            BehaviorType::Adversarial => self.generate_adversarial_actions(),
            BehaviorType::Impatient => self.generate_impatient_actions(),
            BehaviorType::DataHeavy => self.generate_data_heavy_actions(),
            _ => self.generate_standard_actions(persona),
        }
    }

    fn generate_adversarial_actions(&self) -> Vec<UserAction> {
        vec![
            UserAction {
                action_type: ActionType::Navigate,
                target: "/".into(),
                payload: None,
                delay_ms: 100,
                description: "Navigate to home page".into(),
            },
            UserAction {
                action_type: ActionType::Type,
                target: "search_input".into(),
                payload: Some("<script>alert('xss')</script>".into()),
                delay_ms: 200,
                description: "XSS attempt in search field".into(),
            },
            UserAction {
                action_type: ActionType::Type,
                target: "login_email".into(),
                payload: Some("' OR 1=1 --".into()),
                delay_ms: 150,
                description: "SQL injection attempt in login".into(),
            },
            UserAction {
                action_type: ActionType::Submit,
                target: "login_form".into(),
                payload: None,
                delay_ms: 100,
                description: "Submit malicious login form".into(),
            },
            UserAction {
                action_type: ActionType::Upload,
                target: "file_input".into(),
                payload: Some("10GB_file.bin".into()),
                delay_ms: 50,
                description: "Upload oversized file".into(),
            },
            UserAction {
                action_type: ActionType::LargePayload,
                target: "/api/data".into(),
                payload: Some("x".repeat(10_000_000)),
                delay_ms: 10,
                description: "Send 10MB payload to API".into(),
            },
        ]
    }

    fn generate_impatient_actions(&self) -> Vec<UserAction> {
        vec![
            UserAction {
                action_type: ActionType::Navigate,
                target: "/".into(),
                payload: None,
                delay_ms: 50,
                description: "Navigate to home".into(),
            },
            UserAction {
                action_type: ActionType::RapidClick,
                target: "submit_button".into(),
                payload: None,
                delay_ms: 10,
                description: "Rapid-click submit 5 times".into(),
            },
            UserAction {
                action_type: ActionType::Navigate,
                target: "/dashboard".into(),
                payload: None,
                delay_ms: 20,
                description: "Navigate before page loads".into(),
            },
            UserAction {
                action_type: ActionType::Refresh,
                target: "/dashboard".into(),
                payload: None,
                delay_ms: 30,
                description: "Refresh immediately".into(),
            },
            UserAction {
                action_type: ActionType::BackNavigation,
                target: "/".into(),
                payload: None,
                delay_ms: 15,
                description: "Back-navigate rapidly".into(),
            },
            UserAction {
                action_type: ActionType::RapidClick,
                target: "delete_button".into(),
                payload: None,
                delay_ms: 5,
                description: "Rage-click delete button".into(),
            },
        ]
    }

    fn generate_data_heavy_actions(&self) -> Vec<UserAction> {
        vec![
            UserAction {
                action_type: ActionType::Navigate,
                target: "/".into(),
                payload: None,
                delay_ms: 500,
                description: "Navigate to home".into(),
            },
            UserAction {
                action_type: ActionType::Submit,
                target: "/api/batch-create".into(),
                payload: Some("1000 records".into()),
                delay_ms: 1000,
                description: "Batch create 1000 records".into(),
            },
            UserAction {
                action_type: ActionType::Navigate,
                target: "/export".into(),
                payload: None,
                delay_ms: 200,
                description: "Navigate to export page".into(),
            },
            UserAction {
                action_type: ActionType::Click,
                target: "export_all_button".into(),
                payload: None,
                delay_ms: 100,
                description: "Export all data".into(),
            },
            UserAction {
                action_type: ActionType::Upload,
                target: "import_input".into(),
                payload: Some("500MB_dataset.csv".into()),
                delay_ms: 2000,
                description: "Upload large CSV for import".into(),
            },
            UserAction {
                action_type: ActionType::ConcurrentRequest,
                target: "/api/search".into(),
                payload: Some("50 simultaneous queries".into()),
                delay_ms: 50,
                description: "Fire 50 concurrent search requests".into(),
            },
        ]
    }

    fn generate_standard_actions(&self, persona: &UserPersona) -> Vec<UserAction> {
        let base_delay = if persona.patience_level > 0.5 {
            500
        } else {
            100
        };
        vec![
            UserAction {
                action_type: ActionType::Navigate,
                target: "/".into(),
                payload: None,
                delay_ms: base_delay,
                description: "Navigate to home page".into(),
            },
            UserAction {
                action_type: ActionType::Click,
                target: "login_button".into(),
                payload: None,
                delay_ms: base_delay,
                description: "Click login button".into(),
            },
            UserAction {
                action_type: ActionType::Type,
                target: "email_input".into(),
                payload: Some("user@example.com".into()),
                delay_ms: base_delay * 2,
                description: "Type email".into(),
            },
            UserAction {
                action_type: ActionType::Submit,
                target: "login_form".into(),
                payload: None,
                delay_ms: base_delay,
                description: "Submit login".into(),
            },
            UserAction {
                action_type: ActionType::Navigate,
                target: "/dashboard".into(),
                payload: None,
                delay_ms: base_delay,
                description: "Navigate to dashboard".into(),
            },
            UserAction {
                action_type: ActionType::Scroll,
                target: "main_content".into(),
                payload: None,
                delay_ms: base_delay * 3,
                description: "Scroll through content".into(),
            },
        ]
    }

    /// Build the prompt for LLM-generated persona creation.
    pub fn build_persona_prompt(&self, app_type: &str, count: u32) -> (String, String) {
        let system = "You are a QA engineer creating user personas for stress testing.".to_string();
        let user = format!(
            "Generate {count} different user personas for a {app_type} app.\n\
             Each persona should have:\n\
             - name: descriptive name\n\
             - behavior_type: PowerUser, Novice, Adversarial, Mobile, SlowConnection, Impatient, or DataHeavy\n\
             - description: what they do and how they use the app\n\
             - patience_level: 0-1\n\
             - tech_savviness: 0-1\n\
             Return as JSON array matching the UserPersona schema."
        );
        (system, user)
    }

    /// Evaluate a stress report — does it pass?
    pub fn evaluate_report(&self, report: &StressReport) -> bool {
        if self.fail_on_critical {
            let has_critical = report
                .failures
                .iter()
                .any(|f| f.severity == Severity::Critical);
            if has_critical {
                return false;
            }
        }
        report.performance_metrics.error_rate < 0.05
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stress_simulator_default() {
        let sim = StressSimulator::default();
        assert_eq!(sim.num_users, 100);
        assert_eq!(sim.max_actions_per_user, 20);
        assert!(sim.fail_on_critical);
    }

    #[test]
    fn test_generate_default_personas() {
        let sim = StressSimulator::new(5);
        let personas = sim.generate_default_personas(5);
        assert_eq!(personas.len(), 5);

        let types: Vec<&BehaviorType> = personas.iter().map(|p| &p.behavior_type).collect();
        assert!(types.contains(&&BehaviorType::PowerUser));
        assert!(types.contains(&&BehaviorType::Novice));
        assert!(types.contains(&&BehaviorType::Adversarial));
    }

    #[test]
    fn test_generate_actions_adversarial() {
        let sim = StressSimulator::default();
        let persona = UserPersona {
            name: "Adversarial".into(),
            behavior_type: BehaviorType::Adversarial,
            description: "Tries to break things".into(),
            patience_level: 1.0,
            tech_savviness: 0.99,
        };
        let actions = sim.generate_actions_for_persona(&persona);
        assert!(!actions.is_empty());
        // Should contain XSS/SQL injection attempts
        let has_xss = actions
            .iter()
            .any(|a| a.payload.as_deref().unwrap_or("").contains("script"));
        let has_sqli = actions
            .iter()
            .any(|a| a.payload.as_deref().unwrap_or("").contains("OR 1=1"));
        assert!(has_xss);
        assert!(has_sqli);
    }

    #[test]
    fn test_generate_actions_impatient() {
        let sim = StressSimulator::default();
        let persona = UserPersona {
            name: "Impatient".into(),
            behavior_type: BehaviorType::Impatient,
            description: "Rage clicker".into(),
            patience_level: 0.1,
            tech_savviness: 0.6,
        };
        let actions = sim.generate_actions_for_persona(&persona);
        let has_rapid = actions
            .iter()
            .any(|a| a.action_type == ActionType::RapidClick);
        assert!(has_rapid);
    }

    #[test]
    fn test_evaluate_report_passes() {
        let sim = StressSimulator::default();
        let report = StressReport {
            total_personas: 5,
            total_actions: 30,
            actions_completed: 29,
            failures: vec![],
            performance_metrics: PerformanceMetrics {
                avg_response_ms: 150.0,
                p95_response_ms: 500.0,
                p99_response_ms: 800.0,
                max_response_ms: 1200.0,
                error_rate: 0.01,
                total_requests: 100,
            },
            passed: true,
        };
        assert!(sim.evaluate_report(&report));
    }

    #[test]
    fn test_evaluate_report_fails_critical() {
        let sim = StressSimulator::default();
        let report = StressReport {
            total_personas: 5,
            total_actions: 30,
            actions_completed: 25,
            failures: vec![StressFailure {
                persona_name: "Adversarial Alex".into(),
                action_index: 3,
                action_description: "SQL injection".into(),
                failure_type: FailureType::Crash,
                error_message: "Server crashed".into(),
                severity: Severity::Critical,
            }],
            performance_metrics: PerformanceMetrics {
                avg_response_ms: 200.0,
                p95_response_ms: 600.0,
                p99_response_ms: 900.0,
                max_response_ms: 1500.0,
                error_rate: 0.02,
                total_requests: 100,
            },
            passed: false,
        };
        assert!(!sim.evaluate_report(&report));
    }

    #[test]
    fn test_evaluate_report_fails_high_error_rate() {
        let sim = StressSimulator::default();
        let report = StressReport {
            total_personas: 5,
            total_actions: 30,
            actions_completed: 20,
            failures: vec![],
            performance_metrics: PerformanceMetrics {
                avg_response_ms: 200.0,
                p95_response_ms: 600.0,
                p99_response_ms: 900.0,
                max_response_ms: 1500.0,
                error_rate: 0.10,
                total_requests: 100,
            },
            passed: false,
        };
        assert!(!sim.evaluate_report(&report));
    }

    #[test]
    fn test_build_persona_prompt() {
        let sim = StressSimulator::default();
        let (system, user) = sim.build_persona_prompt("e-commerce", 10);
        assert!(system.contains("QA engineer"));
        assert!(user.contains("e-commerce"));
        assert!(user.contains("10"));
    }
}
