use crate::workspace::{HarnessComponent, HarnessWorkspace};
use crate::HarnessError;
use serde::{Deserialize, Serialize};

/// Middleware configuration parsed from `plugins/manifest.md`.
///
/// Supported directives:
/// - `ENABLE_PLUGIN: repair_trailing_commas`
/// - `ENABLE_PLUGIN: strip_thinking_tags`
/// - `MAX_PROMPT_CHARS: <usize>`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPolicy {
    pub repair_trailing_commas: bool,
    pub strip_thinking_tags: bool,
    pub max_prompt_chars: Option<usize>,
}

/// Executes pre-prompt and post-response middleware transforms configured in `plugins/manifest.md`.
pub struct PluginRunner;

impl PluginRunner {
    /// Parses directive lines from `plugins/manifest.md`.
    pub fn parse_policy(plugins_markdown: &str) -> PluginPolicy {
        let mut policy = PluginPolicy::default();

        for raw_line in plugins_markdown.lines() {
            let line = raw_line.trim().trim_start_matches('-').trim();
            if let Some(plugin_name) = line.strip_prefix("ENABLE_PLUGIN:") {
                match plugin_name.trim() {
                    "repair_trailing_commas" => policy.repair_trailing_commas = true,
                    "strip_thinking_tags" => policy.strip_thinking_tags = true,
                    _ => {}
                }
            } else if let Some(limit_str) = line.strip_prefix("MAX_PROMPT_CHARS:") {
                if let Ok(limit) = limit_str.trim().parse::<usize>() {
                    policy.max_prompt_chars = Some(limit);
                }
            }
        }

        policy
    }

    /// Applies pre-prompt transforms (such as `MAX_PROMPT_CHARS` truncation).
    pub fn apply_pre_prompt(
        workspace: &HarnessWorkspace,
        prompt: &str,
    ) -> Result<String, HarnessError> {
        let markdown = workspace.read_component(HarnessComponent::Plugins)?;
        let policy = Self::parse_policy(&markdown);

        if let Some(max_chars) = policy.max_prompt_chars {
            Ok(prompt.chars().take(max_chars).collect())
        } else {
            Ok(prompt.to_string())
        }
    }

    fn strip_think_blocks(input: &str) -> String {
        let mut current = input.to_string();
        while let Some(start) = current.find("<think>") {
            if let Some(end_rel) = current[start..].find("</think>") {
                let end = start + end_rel + "</think>".len();
                current.replace_range(start..end, "");
            } else {
                break;
            }
        }
        current.trim().to_string()
    }

    fn remove_trailing_json_commas(input: &str) -> String {
        let chars: Vec<char> = input.chars().collect();
        let mut out = String::with_capacity(input.len());

        for i in 0..chars.len() {
            if chars[i] == ',' {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                    continue;
                }
            }
            out.push(chars[i]);
        }

        out
    }

    /// Applies post-response transforms (such as stripping `<think>...</think>` blocks
    /// and repairing trailing JSON commas `,}` or `,]` emitted by compact models).
    pub fn apply_post_response(
        workspace: &HarnessWorkspace,
        raw_response: &str,
    ) -> Result<String, HarnessError> {
        let markdown = workspace.read_component(HarnessComponent::Plugins)?;
        let policy = Self::parse_policy(&markdown);

        let mut processed = raw_response.to_string();
        if policy.strip_thinking_tags {
            processed = Self::strip_think_blocks(&processed);
        }
        if policy.repair_trailing_commas {
            processed = Self::remove_trailing_json_commas(&processed);
        }

        Ok(processed)
    }
}
