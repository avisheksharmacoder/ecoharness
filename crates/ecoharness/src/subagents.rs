use crate::workspace::{HarnessComponent, HarnessWorkspace};
use crate::HarnessError;
use serde::{Deserialize, Serialize};

/// Definition of a specialized subagent inside `subagents.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentSpec {
    pub name: String,
    pub model: String,
    pub system_prompt: String,
}

/// Top-level JSON structure of `subagents.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentManifest {
    #[serde(default)]
    pub subagents: Vec<SubagentSpec>,
}

/// Resolves and dispatches `subagent:<name>` tool calls against `subagents.json`.
pub struct SubagentDispatcher;

impl SubagentDispatcher {
    /// Parses `subagents.json` into a typed `SubagentManifest`.
    pub fn parse_manifest(subagents_json: &str) -> Result<SubagentManifest, HarnessError> {
        serde_json::from_str::<SubagentManifest>(subagents_json).map_err(|err| {
            HarnessError::InvalidComponentFormat {
                component: HarnessComponent::Subagents,
                reason: err.to_string(),
            }
        })
    }

    /// If `tool_name` is prefixed with `"subagent:"`, looks up the matching `SubagentSpec`
    /// in the workspace's `subagents.json`. Returns `Ok(None)` for non-subagent tools.
    pub fn resolve_subagent(
        workspace: &HarnessWorkspace,
        tool_name: &str,
    ) -> Result<Option<SubagentSpec>, HarnessError> {
        let target_name = match tool_name.strip_prefix("subagent:") {
            Some(name) if !name.trim().is_empty() => name.trim(),
            _ => return Ok(None),
        };

        let raw_json = workspace.read_component(HarnessComponent::Subagents)?;
        let manifest = Self::parse_manifest(&raw_json)?;

        Ok(manifest
            .subagents
            .into_iter()
            .find(|s| s.name == target_name))
    }
}
