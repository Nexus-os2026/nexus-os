//! Genesis Engine — the core orchestrator for autonomous agent creation.
//!
//! Coordinates gap analysis, agent generation, testing, and deployment.
//! The engine is LLM-agnostic: it builds prompts and parses responses,
//! but the caller is responsible for the actual LLM calls.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::genome::JsonAgentManifest;

use super::deployer;
use super::gap_analysis::{self, ExistingAgentSummary, GapAnalysis};
use super::generator::{self, AgentSpec};
use super::memory::{CreationPattern, PatternStore};
use super::tester::{self, TestTask};

/// Result of a complete Genesis creation cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisResult {
    pub agent_spec: AgentSpec,
    pub manifest_path: String,
    pub test_score: f64,
    pub test_response_preview: String,
    pub creation_time_ms: u64,
    pub iterations: u32,
    pub pattern_reused: bool,
}

/// The Genesis Engine orchestrates autonomous agent creation.
pub struct GenesisEngine {
    base_dir: std::path::PathBuf,
}

impl GenesisEngine {
    /// Create a new Genesis engine rooted at the given project directory.
    pub fn new(base_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Step 1: Build the gap analysis prompt.
    ///
    /// The caller sends this to an LLM and passes the response to
    /// [`parse_gap_analysis`].
    pub fn analyze_gap(
        &self,
        user_request: &str,
        existing_agents: &[ExistingAgentSummary],
    ) -> String {
        gap_analysis::build_gap_analysis_prompt(user_request, existing_agents)
    }

    /// Parse the LLM response from gap analysis.
    pub fn parse_gap_analysis(
        &self,
        user_request: &str,
        llm_response: &str,
    ) -> Result<GapAnalysis, String> {
        gap_analysis::parse_gap_analysis_response(user_request, llm_response)
    }

    /// Step 2: Check if a similar agent was created before (pattern reuse).
    pub fn check_pattern_reuse(&self, analysis: &GapAnalysis) -> Option<(CreationPattern, f64)> {
        let store = PatternStore::load(&self.base_dir);
        store
            .find_similar(
                &analysis.required_capabilities,
                &analysis.missing_capabilities,
            )
            .map(|(pattern, score)| (pattern.clone(), score))
    }

    /// Step 3: Build the system prompt generation request.
    ///
    /// Returns the LLM prompt to send. The caller sends it to the LLM.
    pub fn build_system_prompt_request(&self, spec: &AgentSpec) -> String {
        generator::build_system_prompt_generation_prompt(spec)
    }

    /// Step 3b: Finalize the agent manifest with the generated system prompt.
    pub fn finalize_manifest(
        &self,
        spec: &mut AgentSpec,
        generated_system_prompt: &str,
    ) -> JsonAgentManifest {
        spec.system_prompt = generated_system_prompt.trim().to_string();
        generator::generate_manifest(spec)
    }

    /// Step 4a: Build test task generation prompt.
    pub fn build_test_generation_request(&self, spec: &AgentSpec) -> String {
        tester::build_test_generation_prompt(&spec.name, &spec.capabilities, &spec.description)
    }

    /// Step 4b: Build the prompt to run a test task through the agent.
    pub fn build_agent_test_prompt(
        &self,
        system_prompt: &str,
        task_prompt: &str,
    ) -> (String, String) {
        (system_prompt.to_string(), task_prompt.to_string())
    }

    /// Step 4c: Build the scoring prompt.
    pub fn build_scoring_request(
        &self,
        task_prompt: &str,
        criteria: &[String],
        agent_response: &str,
    ) -> String {
        tester::build_scoring_prompt(task_prompt, criteria, agent_response)
    }

    /// Step 4d: Build the improvement prompt if tests fail.
    pub fn build_improvement_request(
        &self,
        current_prompt: &str,
        test_tasks: &[TestTask],
    ) -> String {
        tester::build_improvement_prompt(current_prompt, test_tasks)
    }

    /// Step 5: Deploy — save manifest and genome to disk.
    pub fn deploy(
        &self,
        spec: &AgentSpec,
        manifest: &JsonAgentManifest,
    ) -> Result<GenesisResult, String> {
        let start = Instant::now();

        // Validate
        deployer::validate_manifest(manifest)?;

        // Save manifest
        let manifest_path = deployer::save_manifest(&self.base_dir, manifest)?;

        // Save genome
        let genome_json = generator::generate_genome_json(spec)?;
        deployer::save_genome(&self.base_dir, &spec.name, &genome_json)?;

        let elapsed = start.elapsed().as_millis() as u64;

        Ok(GenesisResult {
            agent_spec: spec.clone(),
            manifest_path: manifest_path.to_string_lossy().to_string(),
            test_score: 0.0, // Filled in by the orchestrating caller
            test_response_preview: String::new(),
            creation_time_ms: elapsed,
            iterations: 0,
            pattern_reused: false,
        })
    }

    /// Step 6: Store a successful creation pattern for future reuse.
    pub fn store_creation_pattern(
        &self,
        spec: &AgentSpec,
        missing_capabilities: &[String],
        test_score: f64,
    ) -> Result<(), String> {
        let mut store = PatternStore::load(&self.base_dir);

        let pattern = CreationPattern {
            trigger_keywords: missing_capabilities.to_vec(),
            gap_type: spec.category.clone(),
            agent_spec: spec.clone(),
            test_score,
            times_reused: 0,
        };

        store.store_pattern(pattern);
        store.save(&self.base_dir)
    }

    /// List all generated agents.
    pub fn list_generated(&self) -> Result<Vec<JsonAgentManifest>, String> {
        deployer::list_generated_manifests(&self.base_dir)
    }

    /// Delete a generated agent.
    pub fn delete_generated(&self, agent_name: &str) -> Result<(), String> {
        deployer::delete_generated_agent(&self.base_dir, agent_name)
    }

    /// Parse test task definitions from LLM response.
    pub fn parse_test_tasks(&self, response: &str) -> Result<Vec<tester::TestTaskDef>, String> {
        tester::parse_test_tasks(response)
    }

    /// Parse a scoring response from LLM.
    pub fn parse_scoring(&self, response: &str) -> Result<tester::ScoringResult, String> {
        tester::parse_scoring_response(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = GenesisEngine::new(tmp.path());

        // Step 1: Gap analysis prompt
        let agents = vec![ExistingAgentSummary {
            name: "nexus-forge".to_string(),
            description: "Code generation".to_string(),
            capabilities: vec!["fs.read".to_string()],
            autonomy_level: 3,
        }];
        let prompt = engine.analyze_gap("design a 3D game", &agents);
        assert!(prompt.contains("3D game"));

        // Step 3: Build system prompt request
        let spec = AgentSpec {
            name: "nexus-gamewright".to_string(),
            display_name: "GameWright".to_string(),
            description: "3D game specialist".to_string(),
            system_prompt: String::new(),
            autonomy_level: 3,
            capabilities: vec!["fs.read".to_string(), "fs.write".to_string()],
            tools: vec!["fs.read".to_string()],
            category: "creative".to_string(),
            reasoning_strategy: "chain_of_thought".to_string(),
            temperature: 0.8,
            parent_agents: Vec::new(),
        };
        let prompt = engine.build_system_prompt_request(&spec);
        assert!(prompt.contains("GameWright"));

        // Step 3b: Finalize
        let mut spec = spec;
        let manifest =
            engine.finalize_manifest(&mut spec, "You are GameWright, a 3D game specialist.");
        assert_eq!(manifest.name, "nexus-gamewright");
        assert!(manifest.description.contains("GameWright"));

        // Step 5: Deploy
        let result = engine.deploy(&spec, &manifest).unwrap();
        assert!(result.manifest_path.contains("nexus-gamewright"));

        // Step 6: Store pattern
        engine
            .store_creation_pattern(&spec, &["3d_rendering".to_string()], 8.0)
            .unwrap();

        // Verify pattern stored
        let store = PatternStore::load(tmp.path());
        assert_eq!(store.patterns.len(), 1);

        // List generated
        let generated = engine.list_generated().unwrap();
        assert_eq!(generated.len(), 1);

        // Delete
        engine.delete_generated("nexus-gamewright").unwrap();
        let generated = engine.list_generated().unwrap();
        assert!(generated.is_empty());
    }

    #[test]
    fn pattern_reuse_detection() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = GenesisEngine::new(tmp.path());

        // Store a pattern
        let spec = AgentSpec {
            name: "nexus-dbtuner".to_string(),
            display_name: "DB Tuner".to_string(),
            description: "Database optimization".to_string(),
            system_prompt: "You are DB Tuner.".to_string(),
            autonomy_level: 3,
            capabilities: vec![
                "fs.read".to_string(),
                "fs.write".to_string(),
                "llm.query".to_string(),
            ],
            tools: vec!["fs.read".to_string()],
            category: "data".to_string(),
            reasoning_strategy: "chain_of_thought".to_string(),
            temperature: 0.7,
            parent_agents: Vec::new(),
        };
        engine
            .store_creation_pattern(
                &spec,
                &[
                    "database".to_string(),
                    "sql".to_string(),
                    "query_optimization".to_string(),
                ],
                8.0,
            )
            .unwrap();

        // Check reuse with similar request
        let analysis = GapAnalysis {
            user_request: "optimize my database".to_string(),
            required_capabilities: vec![
                "fs.read".to_string(),
                "fs.write".to_string(),
                "llm.query".to_string(),
            ],
            closest_existing_agents: Vec::new(),
            missing_capabilities: vec![
                "database".to_string(),
                "sql".to_string(),
                "query_optimization".to_string(),
            ],
            gap_found: true,
            recommended_agent_spec: None,
        };
        let reuse = engine.check_pattern_reuse(&analysis);
        assert!(reuse.is_some());
    }
}
