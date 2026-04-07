//! Web builder agent: natural language website generation with templates, 3D scenes, and deploy helpers.

pub mod assembler;
// Phase 8A: Supabase backend integration
pub mod backend;
// Phase 14: Multi-user collaboration
pub mod budget;
pub mod build_orchestrator;
pub mod build_stream;
pub mod checkpoint;
pub mod classifier;
pub mod codegen;
pub mod collab;
pub mod content_gen;
pub mod content_payload;
pub mod content_prompt;
pub mod dependency_manifest;
pub mod deploy;
// Phase 10: Stitch MCP + Design Import
pub mod design_import;
pub mod dev_server;
// Phase 9A: Quality Critic — six automated quality checks
pub mod editor_bridge_plugin;
// Phase 13: Image Generation
pub mod image_gen;
pub mod interpreter;
pub mod llm_codegen;
pub mod model_config;
pub mod model_router;
pub mod plan;
pub mod preview;
pub mod project;
pub mod quality;
pub mod react_components;
pub mod react_gen;
pub mod react_iterate;
pub mod slot_schema;
pub mod smart_iterate;
pub mod styles;
pub mod templates;
// Phase 12: Design System + Themes
pub mod theme;
pub mod theme_extract;
pub mod theme_presets;
pub mod threejs;
pub mod token_tailwind;
pub mod tokens;
// Phase 15: Enterprise Trust Pack
pub mod trust_pack;
pub mod variant;
pub mod variant_gen;
pub mod variant_select;
pub mod variant_select_diverse;
// Phase 16: Self-Improving Builder
pub mod self_improve;
pub mod visual_edit;
