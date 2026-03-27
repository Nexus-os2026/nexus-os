use crate::artifacts::ProjectArtifact;
use crate::pipeline::PipelineStage;
use crate::project::{EventType, Project, ProjectStatus};
use crate::quality::{QualityGate, QualityGateResult};
use crate::roles::FactoryRole;
use crate::roles::TeamMember;

/// The Software Factory — orchestrates the full SDLC pipeline.
pub struct SoftwareFactory {
    projects: Vec<Project>,
    history: Vec<Project>,
    max_concurrent: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum FactoryError {
    #[error("Maximum {0} concurrent projects")]
    ConcurrencyLimit(usize),
    #[error("Project not found: {0}")]
    ProjectNotFound(String),
    #[error("Insufficient autonomy for {role}: requires L{required}+, has L{actual}")]
    InsufficientAutonomy {
        role: String,
        required: u8,
        actual: u8,
    },
    #[error("Role already filled: {0}")]
    RoleAlreadyFilled(String),
    #[error("Incomplete team, missing: {missing:?}")]
    IncompleteTeam { missing: Vec<String> },
    #[error("Wrong stage: expected {expected:?}, got {got:?}")]
    WrongStage {
        expected: PipelineStage,
        got: PipelineStage,
    },
    #[error("Governance denied: {0}")]
    GovernanceDenied(String),
}

impl SoftwareFactory {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            projects: Vec::new(),
            history: Vec::new(),
            max_concurrent,
        }
    }

    pub fn create_project(
        &mut self,
        title: String,
        user_request: String,
    ) -> Result<String, FactoryError> {
        if self.projects.len() >= self.max_concurrent {
            return Err(FactoryError::ConcurrencyLimit(self.max_concurrent));
        }
        let project = Project::new(title, user_request);
        let id = project.id.clone();
        self.projects.push(project);
        Ok(id)
    }

    pub fn assign_team_member(
        &mut self,
        project_id: &str,
        agent_id: &str,
        agent_name: &str,
        role: FactoryRole,
        autonomy_level: u8,
        capability_score: Option<f64>,
    ) -> Result<(), FactoryError> {
        let project = self.get_project_mut(project_id)?;

        if autonomy_level < role.min_autonomy() {
            return Err(FactoryError::InsufficientAutonomy {
                role: role.display_name().into(),
                required: role.min_autonomy(),
                actual: autonomy_level,
            });
        }

        // Allow multiple developers, but not duplicate other roles
        if role != FactoryRole::Developer && project.team.iter().any(|m| m.role == role) {
            return Err(FactoryError::RoleAlreadyFilled(role.display_name().into()));
        }

        project.team.push(TeamMember {
            agent_id: agent_id.into(),
            agent_name: agent_name.into(),
            role,
            autonomy_level,
            capability_score,
            assigned_at: epoch_now(),
        });
        Ok(())
    }

    pub fn start_pipeline(&mut self, project_id: &str) -> Result<(), FactoryError> {
        let project = self.get_project_mut(project_id)?;

        let has_pm = project
            .team
            .iter()
            .any(|m| m.role == FactoryRole::ProductManager);
        let has_arch = project
            .team
            .iter()
            .any(|m| m.role == FactoryRole::Architect);
        let has_dev = project
            .team
            .iter()
            .any(|m| m.role == FactoryRole::Developer);
        let has_qa = project
            .team
            .iter()
            .any(|m| m.role == FactoryRole::QualityAssurance);

        if !has_pm || !has_arch || !has_dev || !has_qa {
            return Err(FactoryError::IncompleteTeam {
                missing: [
                    if has_pm { None } else { Some("ProductManager") },
                    if has_arch { None } else { Some("Architect") },
                    if has_dev { None } else { Some("Developer") },
                    if has_qa {
                        None
                    } else {
                        Some("QualityAssurance")
                    },
                ]
                .into_iter()
                .flatten()
                .map(|s| s.into())
                .collect(),
            });
        }

        project.status = ProjectStatus::InProgress;
        project.current_stage = PipelineStage::Requirements;
        project.add_event(
            PipelineStage::Requirements,
            "factory",
            EventType::StageStarted,
            "Pipeline started at Requirements stage",
        );
        Ok(())
    }

    pub fn submit_artifact(
        &mut self,
        project_id: &str,
        artifact: ProjectArtifact,
    ) -> Result<QualityGateResult, FactoryError> {
        let project = self.get_project_mut(project_id)?;

        if artifact.stage != project.current_stage {
            return Err(FactoryError::WrongStage {
                expected: project.current_stage,
                got: artifact.stage,
            });
        }

        let gate_result = QualityGate::evaluate(&artifact);

        project.add_event(
            artifact.stage,
            &artifact.produced_by,
            EventType::ArtifactProduced,
            &format!(
                "{} produced (quality: {:.1}%)",
                artifact.artifact_type,
                gate_result.score * 100.0
            ),
        );

        project.artifacts.push(artifact);
        project.quality_gates.push(gate_result.clone());

        if gate_result.passed {
            project.add_event(
                project.current_stage,
                "factory",
                EventType::QualityGatePassed,
                "Quality gate passed",
            );

            if let Some(next) = project.current_stage.next() {
                project.current_stage = next;
                project.add_event(
                    next,
                    "factory",
                    EventType::StageStarted,
                    &format!("{} started", next.display_name()),
                );
            } else {
                project.status = ProjectStatus::Completed;
                project.completed_at = Some(epoch_now());
                project.add_event(
                    PipelineStage::Verification,
                    "factory",
                    EventType::StageCompleted,
                    "Pipeline completed successfully",
                );
            }
        } else {
            project.status = ProjectStatus::QualityGateHold {
                stage: project.current_stage,
                reason: gate_result.blocking_issues.join("; "),
            };
            project.add_event(
                project.current_stage,
                "factory",
                EventType::QualityGateFailed,
                &format!(
                    "Quality gate failed: {}",
                    gate_result.blocking_issues.join("; ")
                ),
            );
        }

        Ok(gate_result)
    }

    pub fn project_cost(&self, project_id: &str) -> Result<u64, FactoryError> {
        let project = self.get_project(project_id)?;
        Ok(PipelineStage::all()
            .iter()
            .filter(|s| **s <= project.current_stage)
            .map(|s| s.base_cost())
            .sum())
    }

    pub fn complete_project(&mut self, project_id: &str) -> Result<(), FactoryError> {
        if let Some(idx) = self.projects.iter().position(|p| p.id == project_id) {
            let project = self.projects.remove(idx);
            self.history.push(project);
            Ok(())
        } else {
            Err(FactoryError::ProjectNotFound(project_id.into()))
        }
    }

    pub fn get_project(&self, project_id: &str) -> Result<&Project, FactoryError> {
        self.projects
            .iter()
            .find(|p| p.id == project_id)
            .or_else(|| self.history.iter().find(|p| p.id == project_id))
            .ok_or_else(|| FactoryError::ProjectNotFound(project_id.into()))
    }

    fn get_project_mut(&mut self, project_id: &str) -> Result<&mut Project, FactoryError> {
        self.projects
            .iter_mut()
            .find(|p| p.id == project_id)
            .ok_or_else(|| FactoryError::ProjectNotFound(project_id.into()))
    }

    pub fn active_projects(&self) -> &[Project] {
        &self.projects
    }

    pub fn completed_projects(&self) -> &[Project] {
        &self.history
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::*;

    fn setup_factory_with_project() -> (SoftwareFactory, String) {
        let mut f = SoftwareFactory::new(3);
        let id = f
            .create_project("Test Project".into(), "Build a REST API".into())
            .unwrap();
        (f, id)
    }

    fn assign_full_team(f: &mut SoftwareFactory, id: &str) {
        f.assign_team_member(id, "pm-1", "PM Agent", FactoryRole::ProductManager, 4, None)
            .unwrap();
        f.assign_team_member(id, "arch-1", "Arch Agent", FactoryRole::Architect, 4, None)
            .unwrap();
        f.assign_team_member(id, "dev-1", "Dev Agent", FactoryRole::Developer, 3, None)
            .unwrap();
        f.assign_team_member(
            id,
            "qa-1",
            "QA Agent",
            FactoryRole::QualityAssurance,
            3,
            None,
        )
        .unwrap();
    }

    fn make_passing_req() -> ArtifactContent {
        ArtifactContent::Requirements {
            user_stories: vec!["As user I want an API".into()],
            acceptance_criteria: vec!["Returns 200 on GET".into()],
            constraints: vec!["Response < 100ms".into()],
            priorities: Vec::new(),
        }
    }

    #[test]
    fn test_project_creation() {
        let (f, id) = setup_factory_with_project();
        let p = f.get_project(&id).unwrap();
        assert_eq!(p.title, "Test Project");
        assert_eq!(p.status, ProjectStatus::Initializing);
    }

    #[test]
    fn test_assign_team() {
        let (mut f, id) = setup_factory_with_project();
        assign_full_team(&mut f, &id);
        assert_eq!(f.get_project(&id).unwrap().team.len(), 4);
    }

    #[test]
    fn test_insufficient_autonomy() {
        let (mut f, id) = setup_factory_with_project();
        let result = f.assign_team_member(&id, "arch-1", "Arch", FactoryRole::Architect, 2, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient"));
    }

    #[test]
    fn test_duplicate_role_rejected() {
        let (mut f, id) = setup_factory_with_project();
        f.assign_team_member(&id, "pm-1", "PM1", FactoryRole::ProductManager, 4, None)
            .unwrap();
        let result = f.assign_team_member(&id, "pm-2", "PM2", FactoryRole::ProductManager, 4, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_developers() {
        let (mut f, id) = setup_factory_with_project();
        f.assign_team_member(&id, "dev-1", "Dev1", FactoryRole::Developer, 3, None)
            .unwrap();
        f.assign_team_member(&id, "dev-2", "Dev2", FactoryRole::Developer, 3, None)
            .unwrap();
        assert_eq!(
            f.get_project(&id)
                .unwrap()
                .team
                .iter()
                .filter(|m| m.role == FactoryRole::Developer)
                .count(),
            2
        );
    }

    #[test]
    fn test_start_requires_minimum_team() {
        let (mut f, id) = setup_factory_with_project();
        // Only PM
        f.assign_team_member(&id, "pm-1", "PM", FactoryRole::ProductManager, 4, None)
            .unwrap();
        assert!(f.start_pipeline(&id).is_err());

        // Add rest
        f.assign_team_member(&id, "arch-1", "Arch", FactoryRole::Architect, 4, None)
            .unwrap();
        f.assign_team_member(&id, "dev-1", "Dev", FactoryRole::Developer, 3, None)
            .unwrap();
        f.assign_team_member(&id, "qa-1", "QA", FactoryRole::QualityAssurance, 3, None)
            .unwrap();
        assert!(f.start_pipeline(&id).is_ok());
    }

    #[test]
    fn test_pipeline_advances_on_pass() {
        let (mut f, id) = setup_factory_with_project();
        assign_full_team(&mut f, &id);
        f.start_pipeline(&id).unwrap();

        let artifact = ProjectArtifact {
            id: "a1".into(),
            project_id: id.clone(),
            artifact_type: "requirements_spec".into(),
            stage: PipelineStage::Requirements,
            produced_by: "pm-1".into(),
            content: make_passing_req(),
            quality_score: None,
            created_at: 0,
            version: 1,
        };
        let result = f.submit_artifact(&id, artifact).unwrap();
        assert!(result.passed);
        assert_eq!(
            f.get_project(&id).unwrap().current_stage,
            PipelineStage::Architecture
        );
    }

    #[test]
    fn test_pipeline_holds_on_fail() {
        let (mut f, id) = setup_factory_with_project();
        assign_full_team(&mut f, &id);
        f.start_pipeline(&id).unwrap();

        let artifact = ProjectArtifact {
            id: "a1".into(),
            project_id: id.clone(),
            artifact_type: "requirements_spec".into(),
            stage: PipelineStage::Requirements,
            produced_by: "pm-1".into(),
            content: ArtifactContent::Requirements {
                user_stories: Vec::new(),
                acceptance_criteria: Vec::new(),
                constraints: Vec::new(),
                priorities: Vec::new(),
            },
            quality_score: None,
            created_at: 0,
            version: 1,
        };
        let result = f.submit_artifact(&id, artifact).unwrap();
        assert!(!result.passed);
        assert!(matches!(
            f.get_project(&id).unwrap().status,
            ProjectStatus::QualityGateHold { .. }
        ));
    }

    #[test]
    fn test_wrong_stage_rejected() {
        let (mut f, id) = setup_factory_with_project();
        assign_full_team(&mut f, &id);
        f.start_pipeline(&id).unwrap();

        let artifact = ProjectArtifact {
            id: "a1".into(),
            project_id: id.clone(),
            artifact_type: "architecture_doc".into(),
            stage: PipelineStage::Architecture,
            produced_by: "arch-1".into(),
            content: ArtifactContent::Text("design".into()),
            quality_score: None,
            created_at: 0,
            version: 1,
        };
        assert!(f.submit_artifact(&id, artifact).is_err());
    }

    #[test]
    fn test_project_cost_calculation() {
        let (mut f, id) = setup_factory_with_project();
        assign_full_team(&mut f, &id);
        f.start_pipeline(&id).unwrap();
        // At Requirements stage: cost = Requirements base cost
        let cost = f.project_cost(&id).unwrap();
        assert_eq!(cost, PipelineStage::Requirements.base_cost());
    }

    #[test]
    fn test_factory_concurrency_limit() {
        let mut f = SoftwareFactory::new(1);
        f.create_project("P1".into(), "req1".into()).unwrap();
        assert!(f.create_project("P2".into(), "req2".into()).is_err());
    }

    #[test]
    fn test_project_completion() {
        let (mut f, id) = setup_factory_with_project();
        assign_full_team(&mut f, &id);
        f.start_pipeline(&id).unwrap();

        // Submit passing artifacts for all 7 stages
        let stages_content: Vec<(PipelineStage, ArtifactContent)> = vec![
            (PipelineStage::Requirements, make_passing_req()),
            (
                PipelineStage::Architecture,
                ArtifactContent::Architecture {
                    components: vec![Component {
                        name: "API".into(),
                        description: "REST".into(),
                        responsibility: "handle requests".into(),
                    }],
                    interactions: vec!["API -> DB".into()],
                    technology_choices: vec![("lang".into(), "Rust".into())],
                    risks: vec!["complexity".into()],
                },
            ),
            (
                PipelineStage::Implementation,
                ArtifactContent::SourceCode {
                    files: vec![CodeFile {
                        path: "main.rs".into(),
                        content: "fn main() {}".into(),
                        language: "rust".into(),
                        lines: 20,
                    }],
                    language: "Rust".into(),
                    framework: None,
                    entry_point: Some("main.rs".into()),
                },
            ),
            (
                PipelineStage::Testing,
                ArtifactContent::TestResults {
                    total: 10,
                    passed: 10,
                    failed: 0,
                    skipped: 0,
                    coverage_percent: Some(85.0),
                    failures: Vec::new(),
                },
            ),
            (
                PipelineStage::Review,
                ArtifactContent::ReviewOutcome {
                    approved: true,
                    comments: Vec::new(),
                    requested_changes: Vec::new(),
                },
            ),
            (
                PipelineStage::Deployment,
                ArtifactContent::DeploymentResult {
                    success: true,
                    environment: "staging".into(),
                    url: Some("https://staging.example.com".into()),
                    logs: "OK".into(),
                },
            ),
            (
                PipelineStage::Verification,
                ArtifactContent::VerificationReport {
                    all_checks_passed: true,
                    checks: vec![VerificationCheck {
                        name: "health".into(),
                        passed: true,
                        details: "200 OK".into(),
                    }],
                },
            ),
        ];

        for (stage, content) in stages_content {
            let a = ProjectArtifact {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: id.clone(),
                artifact_type: stage.output_artifact().into(),
                stage,
                produced_by: "agent".into(),
                content,
                quality_score: None,
                created_at: 0,
                version: 1,
            };
            f.submit_artifact(&id, a).unwrap();
        }

        assert_eq!(f.get_project(&id).unwrap().status, ProjectStatus::Completed);
    }
}
