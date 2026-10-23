pub mod engine;
pub mod evaluator;
pub mod guardrails;
pub mod mounter;
pub mod ollama;
pub mod plugins;
pub mod scripts;
pub mod subagents;
pub mod trajectory;
pub mod workspace;

#[cfg(test)]
mod tests;

pub use engine::{
    CandidateKind, ComponentDiff, CompositionDecision, ExecutionDriver, HarnessEngine,
    HarnessPatch, PpcStageEvent,
};
pub use evaluator::{EvaluationVerdict, JsonFieldType, JsonSchemaEvaluator, TaskEvaluator};
pub use guardrails::{GuardrailEnforcer, GuardrailPolicy};
pub use mounter::{FileMounter, MountOutcome, MountTarget, StagedContextFile};
pub use ollama::{
    LfmComposeResponse, LfmProposeResponse, LfmTaskResponse, ModelRole, OllamaConfig, OllamaDriver,
    OllamaModelInfo,
};
pub use plugins::{PluginPolicy, PluginRunner};
pub use scripts::{ScriptExecutionOutput, ScriptRunner, ScriptToolManifest, ScriptToolSpec};
pub use subagents::{SubagentDispatcher, SubagentManifest, SubagentSpec};
pub use trajectory::{CompressedTrajectory, Trajectory, TrajectoryCompressor, TrajectoryStep};
pub use workspace::{
    ExperienceEntry, HarnessComponent, HarnessSnapshot, HarnessWorkspace,
};

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid format in harness component {component:?}: {reason}")]
    InvalidComponentFormat {
        component: HarnessComponent,
        reason: String,
    },

    #[error("Trajectory compression failed: budget {budget} chars is too small")]
    CompressionBudgetTooSmall { budget: usize },

    #[error("Guardrail blocked tool '{tool_name}': {reason}")]
    GuardrailBlocked { tool_name: String, reason: String },

    #[error("Script execution error for tool '{tool_name}': {reason}")]
    ScriptExecutionError { tool_name: String, reason: String },

    #[error("Subagent error for '{name}': {reason}")]
    SubagentError { name: String, reason: String },

    #[error("Plugin error for '{name}': {reason}")]
    PluginError { name: String, reason: String },

    #[error("Driver execution error: {0}")]
    DriverError(String),
}

// EcoHarness Aliases
pub type EcoHarnessError = HarnessError;
pub type EcoHarnessWorkspace = HarnessWorkspace;
pub type EcoHarnessComponent = HarnessComponent;
pub type EcoHarnessSnapshot = HarnessSnapshot;
pub type EcoHarnessPatch = HarnessPatch;
pub type EcoHarnessEngine<D> = HarnessEngine<D>;
pub type EcoHarnessDriver = OllamaDriver;
pub type EcoHarnessFileMounter = FileMounter;
