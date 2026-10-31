//! Web builder: turns business notes into a local static site from templates.
//! The Mother Nexus cleanup removed publishing, backends, design import, 3D
//! scenes and the network image tiers; nothing here deploys or publishes.

pub mod assembler;
// Phase 14: Multi-user collaboration
pub mod budget;
pub mod build_orchestrator;
pub mod build_stream;
pub mod checkpoint;
pub mod classifier;
pub mod collab;
pub mod content_gen;
pub mod content_payload;
pub mod content_prompt;
pub mod dependency_manifest;
// Phase 9A: Quality Critic — six automated quality checks
// Phase 13: Image Generation
pub mod image_gen;
pub mod model_config;
pub mod model_router;
pub mod plan;
pub mod project;
pub mod quality;
pub mod react_components;
pub mod react_gen;
pub mod slot_schema;
pub mod templates;
pub mod timestamp;
pub mod token_tailwind;
pub mod tokens;
// Phase 15: Enterprise Trust Pack
pub mod trust_pack;
pub mod variant;
pub mod variant_select;
// Phase 16: Self-Improving Builder
pub mod self_improve;
pub mod visual_edit;
