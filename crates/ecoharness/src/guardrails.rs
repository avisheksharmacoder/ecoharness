use crate::workspace::{HarnessComponent, HarnessWorkspace};
use crate::HarnessError;
use serde::{Deserialize, Serialize};

/// Parsed runtime enforcement rules extracted from `guardrails.md`.
///
/// Supported directive lines inside `guardrails.md`:
/// - `DENY_TOOL: <tool_name>`
/// - `DENY_ARG_PATTERN: <substring>`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardrailPolicy {
    pub denied_tools: Vec<String>,
    pub denied_arg_patterns: Vec<String>,
    pub block_path_traversal: bool,
}

/// Inspects proposed tool invocations against `guardrails.md` before any subprocess is spawned.
pub struct GuardrailEnforcer;

impl GuardrailEnforcer {
    /// Parses directive lines (`DENY_TOOL:`, `DENY_ARG_PATTERN:`) from `guardrails.md`.
    /// By default, `block_path_traversal` (`../` or `..\`) is always enabled.
    pub fn parse_policy(guardrails_markdown: &str) -> GuardrailPolicy {
        let mut denied_tools = Vec::new();
        let mut denied_arg_patterns = Vec::new();

        for raw_line in guardrails_markdown.lines() {
            let line = raw_line.trim().trim_start_matches('-').trim();
            if let Some(tool) = line.strip_prefix("DENY_TOOL:") {
                let trimmed = tool.trim();
                if !trimmed.is_empty() {
                    denied_tools.push(trimmed.to_string());
                }
            } else if let Some(pattern) = line.strip_prefix("DENY_ARG_PATTERN:") {
                let trimmed = pattern.trim();
                if !trimmed.is_empty() {
                    denied_arg_patterns.push(trimmed.to_string());
                }
            }
        }

        GuardrailPolicy {
            denied_tools,
            denied_arg_patterns,
            block_path_traversal: true,
        }
    }

    /// Validates a tool call against the workspace's active `guardrails.md`.
    /// Returns `Err(HarnessError::GuardrailBlocked { .. })` if any rule is violated.
    pub fn validate_tool_call(
        workspace: &HarnessWorkspace,
        tool_name: &str,
        tool_input: &str,
    ) -> Result<(), HarnessError> {
        let markdown = workspace.read_component(HarnessComponent::Guardrails)?;
        let policy = Self::parse_policy(&markdown);

        if policy.denied_tools.iter().any(|t| t == tool_name) {
            return Err(HarnessError::GuardrailBlocked {
                tool_name: tool_name.to_string(),
                reason: format!("Tool '{}' is explicitly denied by guardrails.md", tool_name),
            });
        }

        for pattern in &policy.denied_arg_patterns {
            if tool_input.contains(pattern) {
                return Err(HarnessError::GuardrailBlocked {
                    tool_name: tool_name.to_string(),
                    reason: format!(
                        "Tool input contains forbidden argument pattern '{}'",
                        pattern
                    ),
                });
            }
        }

        if policy.block_path_traversal
            && (tool_input.contains("../") || tool_input.contains("..\\"))
        {
            return Err(HarnessError::GuardrailBlocked {
                tool_name: tool_name.to_string(),
                reason: "Relative path traversal ('../' or '..\\') is blocked by guardrails"
                    .to_string(),
            });
        }

        Ok(())
    }
}
