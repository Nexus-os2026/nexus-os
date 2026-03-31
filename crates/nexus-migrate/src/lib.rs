//! Migration tool for importing agents from CrewAI and LangGraph into Nexus OS.
//!
//! # Supported Frameworks
//!
//! - **CrewAI**: Parses `agents.yaml` and `tasks.yaml` configuration files.
//! - **LangGraph**: Parses Python source files containing `StateGraph` definitions.
//!
//! # Example
//!
//! ```
//! use nexus_migrate::crewai::CrewAIParser;
//! use nexus_migrate::output::OutputGenerator;
//!
//! let agents_yaml = r#"
//! researcher:
//!   role: "Data Researcher"
//!   goal: "Find information"
//!   backstory: "Expert researcher"
//!   tools:
//!     - SerperDevTool
//!   llm: gpt-4o
//! "#;
//!
//! let result = CrewAIParser::migrate(agents_yaml, None).unwrap();
//! let output = OutputGenerator::generate_all(&result);
//! assert_eq!(output.agents.len(), 1);
//! ```

pub mod crewai;
pub mod langgraph;
pub mod output;
pub mod tauri_commands;
pub mod tool_map;
pub mod types;
