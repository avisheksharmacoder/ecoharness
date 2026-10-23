use crate::guardrails::GuardrailEnforcer;
use crate::workspace::{HarnessComponent, HarnessWorkspace};
use crate::HarnessError;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

/// Specification for a single executable tool defined in `scripts/manifest.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptToolSpec {
    pub name: String,
    pub description: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Top-level JSON manifest embedded inside `scripts/manifest.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptToolManifest {
    #[serde(default)]
    pub tools: Vec<ScriptToolSpec>,
}

/// Captured stdout, stderr, and exit status from running a tool subprocess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExecutionOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Executes deterministic local scripts/tools mounted in the `Scripts` harness component.
/// Passes `tool_input` via the `HARNESS_TOOL_INPUT` environment variable.
pub struct ScriptRunner;

impl ScriptRunner {
    /// Extracts and parses a `ScriptToolManifest` from `scripts/manifest.md`.
    /// Supports either raw JSON or a ```json ... ``` fenced block inside the Markdown file.
    pub fn parse_manifest(scripts_markdown: &str) -> Result<ScriptToolManifest, HarnessError> {
        let trimmed = scripts_markdown.trim();

        let json_slice = if let Some(fence_start) = trimmed.find("```json") {
            let after_fence = &trimmed[fence_start + "```json".len()..];
            if let Some(fence_end) = after_fence.find("```") {
                after_fence[..fence_end].trim()
            } else {
                after_fence.trim()
            }
        } else if let (Some(first_brace), Some(last_brace)) =
            (trimmed.find('{'), trimmed.rfind('}'))
        {
            &trimmed[first_brace..=last_brace]
        } else {
            return Ok(ScriptToolManifest::default());
        };

        serde_json::from_str::<ScriptToolManifest>(json_slice).map_err(|err| {
            HarnessError::InvalidComponentFormat {
                component: HarnessComponent::Scripts,
                reason: err.to_string(),
            }
        })
    }

    /// Validates the tool call against `GuardrailEnforcer`, looks up `tool_name` in
    /// `scripts/manifest.md`, and runs the subprocess with `HARNESS_TOOL_INPUT=tool_input`.
    pub async fn execute_tool(
        workspace: &HarnessWorkspace,
        tool_name: &str,
        tool_input: &str,
    ) -> Result<ScriptExecutionOutput, HarnessError> {
        GuardrailEnforcer::validate_tool_call(workspace, tool_name, tool_input)?;

        let scripts_md = workspace.read_component(HarnessComponent::Scripts)?;
        let manifest = Self::parse_manifest(&scripts_md)?;

        let spec = manifest
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| HarnessError::ScriptExecutionError {
                tool_name: tool_name.to_string(),
                reason: format!("Tool '{}' is not registered in scripts/manifest.md", tool_name),
            })?;

        let output = Command::new(&spec.program)
            .args(&spec.args)
            .current_dir(workspace.root())
            .env("HARNESS_TOOL_INPUT", tool_input)
            .output()
            .await
            .map_err(|err| HarnessError::ScriptExecutionError {
                tool_name: tool_name.to_string(),
                reason: format!("Failed to spawn '{}': {}", spec.program, err),
            })?;

        Ok(ScriptExecutionOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}
