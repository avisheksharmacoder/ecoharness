use crate::engine::HarnessPatch;
use crate::HarnessError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const EXPERIENCE_FILENAME: &str = "EXPERIENCE.md";
const HISTORY_DIRNAME: &str = ".history";

/// The 8 editable file-mounted components of Google's Harness² architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HarnessComponent {
    SystemPrompt,
    Guardrails,
    Memory,
    ToolGuidance,
    Subagents,
    Skills,
    Scripts,
    Plugins,
}

impl HarnessComponent {
    pub const fn all() -> [HarnessComponent; 8] {
        [
            Self::SystemPrompt,
            Self::Guardrails,
            Self::Memory,
            Self::ToolGuidance,
            Self::Subagents,
            Self::Skills,
            Self::Scripts,
            Self::Plugins,
        ]
    }

    /// Returns the canonical relative file path inside a harness workspace directory.
    pub const fn relative_path(&self) -> &'static str {
        match self {
            Self::SystemPrompt => "system_prompt.md",
            Self::Guardrails => "guardrails.md",
            Self::Memory => "memory.md",
            Self::ToolGuidance => "tool_guidance.md",
            Self::Subagents => "subagents.json",
            Self::Skills => "skills/manifest.md",
            Self::Scripts => "scripts/manifest.md",
            Self::Plugins => "plugins/manifest.md",
        }
    }

    /// Default initial content written when a workspace is first initialized.
    pub const fn default_content(&self) -> &'static str {
        match self {
            Self::SystemPrompt => "# System Prompt\nYou are a precise, tool-using agent.\n",
            Self::Guardrails => "# Guardrails\nNever fabricate tool outputs or bypass schema validation.\n",
            Self::Memory => "# Working Memory\nNo task-specific facts recorded yet.\n",
            Self::ToolGuidance => "# Tool Guidance\nValidate all required parameters before invoking a tool.\n",
            Self::Subagents => "{\n  \"subagents\": []\n}\n",
            Self::Skills => "# Skills Manifest\nNo custom skills mounted.\n",
            Self::Scripts => "# Scripts Manifest\nNo custom scripts mounted.\n",
            Self::Plugins => "# Plugins Manifest\nNo active middleware plugins.\n",
        }
    }
}

/// Deterministic in-memory snapshot of all 8 components for integrity and rollback checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessSnapshot {
    pub files: BTreeMap<HarnessComponent, String>,
}

/// A structured lesson appended to `EXPERIENCE.md` during Harnessing-level recursion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceEntry {
    pub task_summary: String,
    pub adopted_components: Vec<HarnessComponent>,
    pub lesson: String,
}

/// Manages the on-disk 8-component directory and `EXPERIENCE.md` log.
#[derive(Debug, Clone)]
pub struct HarnessWorkspace {
    root: PathBuf,
}

impl HarnessWorkspace {
    /// Initializes a new 8-component workspace on disk with valid default files and `EXPERIENCE.md`.
    pub fn init(root: impl AsRef<Path>) -> Result<Self, HarnessError> {
        let root_path = root.as_ref().to_path_buf();
        fs::create_dir_all(&root_path).map_err(|source| HarnessError::Io {
            path: root_path.clone(),
            source,
        })?;

        let ws = Self { root: root_path };

        for component in HarnessComponent::all() {
            ws.write_component(component, component.default_content())?;
        }

        let exp_path = ws.experience_path();
        if !exp_path.exists() {
            fs::write(&exp_path, "# EcoHarness Experience Log\n\n").map_err(|source| {
                HarnessError::Io {
                    path: exp_path,
                    source,
                }
            })?;
        }

        Ok(ws)
    }

    /// Loads an existing workspace, verifying that all 8 component files and `EXPERIENCE.md` exist.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, HarnessError> {
        let root_path = root.as_ref().to_path_buf();
        let ws = Self { root: root_path };

        for component in HarnessComponent::all() {
            let content = ws.read_component(component)?;
            if component == HarnessComponent::Subagents {
                Self::validate_subagents_json(&content)?;
            }
        }

        let _ = ws.read_experience()?;
        Ok(ws)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn component_path(&self, component: HarnessComponent) -> PathBuf {
        self.root.join(component.relative_path())
    }

    fn experience_path(&self) -> PathBuf {
        self.root.join(EXPERIENCE_FILENAME)
    }

    fn snapshot_path(&self, tag: &str) -> PathBuf {
        let safe_tag: String = tag
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        self.root.join(HISTORY_DIRNAME).join(format!("{}.json", safe_tag))
    }

    fn validate_subagents_json(content: &str) -> Result<(), HarnessError> {
        serde_json::from_str::<serde_json::Value>(content).map_err(|err| {
            HarnessError::InvalidComponentFormat {
                component: HarnessComponent::Subagents,
                reason: err.to_string(),
            }
        })?;
        Ok(())
    }

    /// Reads the current UTF-8 content of one of the 8 harness components.
    pub fn read_component(&self, component: HarnessComponent) -> Result<String, HarnessError> {
        let path = self.component_path(component);
        fs::read_to_string(&path).map_err(|source| HarnessError::Io { path, source })
    }

    /// Writes new content to a component. Validates JSON syntax if `component == Subagents`.
    pub fn write_component(
        &self,
        component: HarnessComponent,
        content: &str,
    ) -> Result<(), HarnessError> {
        if component == HarnessComponent::Subagents {
            Self::validate_subagents_json(content)?;
        }

        let path = self.component_path(component);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| HarnessError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::write(&path, content).map_err(|source| HarnessError::Io { path, source })
    }

    /// Captures an exact snapshot of all 8 components.
    pub fn snapshot(&self) -> Result<HarnessSnapshot, HarnessError> {
        let mut files = BTreeMap::new();
        for component in HarnessComponent::all() {
            let content = self.read_component(component)?;
            files.insert(component, content);
        }
        Ok(HarnessSnapshot { files })
    }

    /// Clones all 8 components and `EXPERIENCE.md` into an isolated directory for candidate probing.
    pub fn clone_to_sandbox(
        &self,
        sandbox_dir: impl AsRef<Path>,
    ) -> Result<HarnessWorkspace, HarnessError> {
        let sandbox_path = sandbox_dir.as_ref().to_path_buf();
        fs::create_dir_all(&sandbox_path).map_err(|source| HarnessError::Io {
            path: sandbox_path.clone(),
            source,
        })?;

        let sandbox_ws = HarnessWorkspace { root: sandbox_path };
        let current_snapshot = self.snapshot()?;

        for (component, content) in current_snapshot.files {
            sandbox_ws.write_component(component, &content)?;
        }

        let exp_content = self.read_experience()?;
        let sandbox_exp_path = sandbox_ws.experience_path();
        fs::write(&sandbox_exp_path, exp_content).map_err(|source| HarnessError::Io {
            path: sandbox_exp_path,
            source,
        })?;

        Ok(sandbox_ws)
    }

    /// Atomically applies a set of component diffs. If any diff fails validation (e.g. invalid JSON
    /// in `Subagents`), rolls back all modified components to their prior state.
    pub fn apply_patch(&self, patch: &HarnessPatch) -> Result<(), HarnessError> {
        let backup = self.snapshot()?;

        for diff in &patch.diffs {
            if let Err(err) = self.write_component(diff.component, &diff.updated_content) {
                // Roll back all previously modified components in this patch
                for (comp, original_content) in &backup.files {
                    let _ = self.write_component(*comp, original_content);
                }
                return Err(err);
            }
        }

        Ok(())
    }

    /// Reads the full content of `EXPERIENCE.md`.
    pub fn read_experience(&self) -> Result<String, HarnessError> {
        let path = self.experience_path();
        fs::read_to_string(&path).map_err(|source| HarnessError::Io { path, source })
    }

    /// Formats and appends a new lesson entry to `EXPERIENCE.md`.
    pub fn append_experience(&self, entry: &ExperienceEntry) -> Result<(), HarnessError> {
        let path = self.experience_path();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| HarnessError::Io {
                path: path.clone(),
                source,
            })?;

        let formatted = format!(
            "## Task: {}\n- **Adopted Components:** {:?}\n- **Lesson:** {}\n\n",
            entry.task_summary, entry.adopted_components, entry.lesson
        );

        file.write_all(formatted.as_bytes())
            .map_err(|source| HarnessError::Io { path, source })
    }

    fn tokenize_keywords(text: &str) -> BTreeSet<String> {
        text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map(|w| w.trim().to_ascii_lowercase())
            .filter(|w| w.len() >= 2)
            .collect()
    }

    /// Retrieves up to `top_k` lessons from `EXPERIENCE.md` ranked by keyword overlap with `query`.
    /// Prevents `EXPERIENCE.md` growth from overflowing `LFM2.5-350M`'s context window.
    pub fn retrieve_relevant_experience(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<String, HarnessError> {
        if top_k == 0 {
            return Ok(String::new());
        }

        let raw = self.read_experience()?;
        let blocks: Vec<String> = raw
            .split("## Task: ")
            .skip(1)
            .map(|b| format!("## Task: {}", b.trim()))
            .collect();

        if blocks.is_empty() {
            return Ok(raw);
        }

        let query_tokens = Self::tokenize_keywords(query);
        let mut scored: Vec<(usize, usize, String)> = blocks
            .into_iter()
            .enumerate()
            .map(|(idx, block)| {
                let block_tokens = Self::tokenize_keywords(&block);
                let overlap = query_tokens.intersection(&block_tokens).count();
                (overlap, idx, block)
            })
            .collect();

        // Sort descending by keyword overlap score, breaking ties by most recent entry
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

        let mut selected = Vec::new();
        for (score, _idx, block) in scored.into_iter().take(top_k) {
            if score > 0 || selected.is_empty() {
                selected.push(block);
            }
        }

        Ok(selected.join("\n\n"))
    }

    /// Saves a named checkpoint of all 8 harness components under `<root>/.history/<tag>.json`.
    pub fn save_versioned_snapshot(&self, tag: &str) -> Result<PathBuf, HarnessError> {
        let snapshot = self.snapshot()?;
        let path = self.snapshot_path(tag);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| HarnessError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let serialized = serde_json::to_string_pretty(&snapshot).map_err(|err| {
            HarnessError::DriverError(format!("Failed to serialize snapshot: {}", err))
        })?;

        fs::write(&path, serialized).map_err(|source| HarnessError::Io {
            path: path.clone(),
            source,
        })?;

        Ok(path)
    }

    /// Restores all 8 harness components from `<root>/.history/<tag>.json`.
    pub fn rollback_to_snapshot(&self, tag: &str) -> Result<(), HarnessError> {
        let path = self.snapshot_path(tag);
        let content = fs::read_to_string(&path).map_err(|source| HarnessError::Io {
            path: path.clone(),
            source,
        })?;

        let snapshot: HarnessSnapshot = serde_json::from_str(&content).map_err(|err| {
            HarnessError::DriverError(format!("Invalid snapshot JSON at {:?}: {}", path, err))
        })?;

        for (component, file_content) in snapshot.files {
            self.write_component(component, &file_content)?;
        }

        Ok(())
    }
}
