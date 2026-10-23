use crate::ollama::OllamaDriver;
use crate::workspace::{HarnessComponent, HarnessWorkspace};
use crate::HarnessError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Defines where an added file should be routed (used by both the SDK and the GUI "Add File" button).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountTarget {
    /// Injects the file content into the user prompt context for LLM turns.
    PromptContext,
    /// Mounts/overwrites the file directly into one of the 8 Harness² component files.
    HarnessSlot(HarnessComponent),
}

/// A user file staged in memory for prompt-context injection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedContextFile {
    pub filename: String,
    pub content: String,
}

/// Result of mounting a file through `FileMounter`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountOutcome {
    StagedInContext(StagedContextFile),
    MountedToComponent(HarnessComponent),
}

/// Routes user-selected files into either the active prompt context or the 8-file harness workspace.
pub struct FileMounter;

impl FileMounter {
    pub fn mount_file(
        workspace: &HarnessWorkspace,
        driver: &OllamaDriver,
        file_path: impl AsRef<Path>,
        target: MountTarget,
    ) -> Result<MountOutcome, HarnessError> {
        let path = file_path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| HarnessError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        match target {
            MountTarget::PromptContext => {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("attached_file")
                    .to_string();

                let staged = StagedContextFile { filename, content };
                driver.stage_context_file(staged.clone())?;
                Ok(MountOutcome::StagedInContext(staged))
            }
            MountTarget::HarnessSlot(component) => {
                workspace.write_component(component, &content)?;
                Ok(MountOutcome::MountedToComponent(component))
            }
        }
    }
}
