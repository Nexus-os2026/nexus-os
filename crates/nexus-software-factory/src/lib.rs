pub mod artifacts;
pub mod economy;
pub mod factory;
pub mod governance;
pub mod pipeline;
pub mod project;
pub mod quality;
pub mod roles;
pub mod tauri_commands;

pub use artifacts::{ArtifactContent, CodeFile, Component, ProjectArtifact};
pub use factory::{FactoryError, SoftwareFactory};
pub use governance::{FactoryPolicy, FACTORY_CAPABILITY};
pub use pipeline::PipelineStage;
pub use project::{Project, ProjectStatus};
pub use quality::{QualityGate, QualityGateResult};
pub use roles::{FactoryRole, TeamMember};
pub use tauri_commands::FactoryState;
