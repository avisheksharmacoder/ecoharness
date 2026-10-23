use crate::engine::{
    CandidateKind, ComponentDiff, CompositionDecision, ExecutionDriver, HarnessPatch,
};
use crate::evaluator::TaskEvaluator;
use crate::mounter::StagedContextFile;
use crate::scripts::ScriptRunner;
use crate::trajectory::{CompressedTrajectory, Trajectory, TrajectoryStep};
use crate::workspace::HarnessWorkspace;
use crate::HarnessError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Assigns how a local Ollama model is used inside the Harness² loop (and GUI detail pane).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelRole {
    /// Runs the task trajectory using the 8 harness files.
    Executor,
    /// Critiques trajectories and proposes/composes harness diffs.
    Editor,
    /// Single compact model (e.g., `lfm2.5-350m`) acts as both Executor and Editor.
    Both,
}

/// Connection and generation parameters for an Ollama backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub executor_model: String,
    pub editor_model: String,
    pub temperature: f32,
    pub enforce_json_schema: bool,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            executor_model: "lfm2.5-350m".to_string(),
            editor_model: "lfm2.5-350m".to_string(),
            temperature: 0.1,
            enforce_json_schema: true,
        }
    }
}

/// Metadata for a locally installed Ollama model returned by `GET /api/tags`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaModelInfo {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: String,
    #[serde(default)]
    pub modified_at: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsEnvelope {
    #[serde(default)]
    models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaChatOptions {
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    options: OllamaChatOptions,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponseEnvelope {
    message: OllamaChatResponseMessage,
}

/// Strict JSON contract returned by `LFM2.5-350M` during task execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LfmTaskResponse {
    pub thought: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_output: Option<String>,
    pub error: Option<String>,
    pub final_output: Option<String>,
    pub succeeded: bool,
    pub score: f32,
}

/// Strict JSON contract returned by `LFM2.5-350M` during the Propose step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfmProposeResponse {
    pub invariant_diffs: Vec<ComponentDiff>,
    pub domain_diffs: Vec<ComponentDiff>,
}

/// Strict JSON contract returned by `LFM2.5-350M` during the Compose step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LfmComposeResponse {
    pub adopted_diffs: Vec<ComponentDiff>,
    pub lesson_learned: String,
    pub winning_score: f32,
    pub final_output: Option<String>,
    pub succeeded: bool,
}

/// HTTP driver connecting Harness² to a local or remote Ollama server.
#[derive(Debug, Clone)]
pub struct OllamaDriver {
    client: reqwest::Client,
    config: OllamaConfig,
    staged_files: Arc<RwLock<Vec<StagedContextFile>>>,
    max_turns: usize,
    enable_script_execution: bool,
    evaluator: Option<Arc<dyn TaskEvaluator>>,
}

impl OllamaDriver {
    pub fn new(config: OllamaConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
            staged_files: Arc::new(RwLock::new(Vec::new())),
            max_turns: 1,
            enable_script_execution: false,
            evaluator: None,
        }
    }

    /// Configures the maximum number of ReAct tool-execution turns per task (`>= 1`).
    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns.max(1);
        self
    }

    /// Enables real subprocess execution of tools mounted in `scripts/manifest.md`
    /// gated by `GuardrailEnforcer`.
    pub fn with_script_execution(mut self, enabled: bool) -> Self {
        self.enable_script_execution = enabled;
        self
    }

    /// Attaches a deterministic `TaskEvaluator` to override self-reported LLM scores.
    pub fn with_evaluator(mut self, evaluator: Arc<dyn TaskEvaluator>) -> Self {
        self.evaluator = Some(evaluator);
        self
    }

    pub fn config(&self) -> &OllamaConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: OllamaConfig) {
        self.config = config;
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }

    /// Queries `GET /api/tags` on the configured Ollama instance to discover available models.
    pub async fn list_models(&self) -> Result<Vec<OllamaModelInfo>, HarnessError> {
        let url = self.endpoint("/api/tags");
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::DriverError(format!("GET {} failed: {}", url, e)))?
            .error_for_status()
            .map_err(|e| HarnessError::DriverError(format!("Status error from {}: {}", url, e)))?;

        let envelope: OllamaTagsEnvelope = resp.json().await.map_err(|e| {
            HarnessError::DriverError(format!("Invalid JSON from /api/tags: {}", e))
        })?;

        Ok(envelope.models)
    }

    /// Stages a file in memory so its contents are injected into the next prompt context.
    pub fn stage_context_file(&self, file: StagedContextFile) -> Result<(), HarnessError> {
        let mut guard = self
            .staged_files
            .write()
            .map_err(|_| HarnessError::DriverError("Staged files lock poisoned".to_string()))?;
        guard.push(file);
        Ok(())
    }

    /// Returns all currently staged prompt-context files.
    pub fn staged_context_files(&self) -> Vec<StagedContextFile> {
        self.staged_files
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Clears staged prompt-context files.
    pub fn clear_staged_context(&self) {
        if let Ok(mut guard) = self.staged_files.write() {
            guard.clear();
        }
    }

    /// Strips optional Markdown code fences (e.g. ```json ... ```) emitted by small models
    /// before deserializing into the target JSON contract.
    pub fn sanitize_json_payload(raw: &str) -> &str {
        let trimmed = raw.trim();
        if let Some(after_open) = trimmed.strip_prefix("```") {
            let after_lang = after_open
                .strip_prefix("json")
                .or_else(|| after_open.strip_prefix("JSON"))
                .unwrap_or(after_open)
                .trim_start();
            if let Some(stripped) = after_lang.strip_suffix("```") {
                return stripped.trim();
            }
        }
        trimmed
    }

    /// Renders all 8 harness components into a compact system prompt block.
    fn build_harness_system_prompt(workspace: &HarnessWorkspace) -> Result<String, HarnessError> {
        let snapshot = workspace.snapshot()?;
        let mut out = String::from("=== MOUNTED HARNESS² 8-COMPONENT STATE ===\n");
        for (component, content) in snapshot.files {
            out.push_str(&format!("\n--- [{:?}] ---\n{}\n", component, content.trim()));
        }
        Ok(out)
    }

    async fn send_chat_json<T: serde::de::DeserializeOwned>(
        &self,
        model: &str,
        system_content: String,
        user_content: String,
    ) -> Result<T, HarnessError> {
        let url = self.endpoint("/api/chat");
        let req_body = OllamaChatRequest {
            model: model.to_string(),
            messages: vec![
                OllamaChatMessage {
                    role: "system".to_string(),
                    content: system_content,
                },
                OllamaChatMessage {
                    role: "user".to_string(),
                    content: user_content,
                },
            ],
            stream: false,
            format: if self.config.enforce_json_schema {
                Some("json".to_string())
            } else {
                None
            },
            options: OllamaChatOptions {
                temperature: self.config.temperature,
            },
        };

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| HarnessError::DriverError(format!("POST {} failed: {}", url, e)))?
            .error_for_status()
            .map_err(|e| HarnessError::DriverError(format!("Status error from {}: {}", url, e)))?;

        let envelope: OllamaChatResponseEnvelope = resp.json().await.map_err(|e| {
            HarnessError::DriverError(format!("Malformed Ollama chat envelope: {}", e))
        })?;

        let clean_json = Self::sanitize_json_payload(&envelope.message.content);
        serde_json::from_str::<T>(clean_json).map_err(|e| {
            HarnessError::DriverError(format!(
                "Failed to parse LFM JSON contract ({}): raw='{}'",
                e, clean_json
            ))
        })
    }

    /// Executes a multi-turn ReAct loop using `GuardrailEnforcer`, `ScriptRunner`,
    /// and optional `TaskEvaluator`.
    async fn execute_multiturn_react(
        &self,
        workspace: &HarnessWorkspace,
        task_prompt: &str,
    ) -> Result<Trajectory, HarnessError> {
        let system_prompt = Self::build_harness_system_prompt(workspace)?;
        let attached_files = self.staged_context_files();

        let mut steps: Vec<TrajectoryStep> = Vec::new();
        let mut final_output: Option<String> = None;
        let mut succeeded = false;
        let mut score = 0.0f32;

        for turn in 1..=self.max_turns {
            let mut user_payload = format!("TASK: {}\n", task_prompt);

            if !attached_files.is_empty() {
                user_payload.push_str("\n=== ATTACHED CONTEXT FILES ===\n");
                for file in &attached_files {
                    user_payload.push_str(&format!(
                        "FILE [{}]:\n{}\n",
                        file.filename, file.content
                    ));
                }
            }

            if !steps.is_empty() {
                user_payload.push_str("\n=== PRIOR TOOL EXECUTION OBSERVATIONS ===\n");
                for prior in &steps {
                    user_payload.push_str(&format!(
                        "TURN {} | tool={:?} | input={:?} | output={:?} | error={:?}\n",
                        prior.turn,
                        prior.tool_name,
                        prior.tool_input,
                        prior.tool_output,
                        prior.error
                    ));
                }
            }

            user_payload.push_str(
                "\nRespond strictly with JSON matching LfmTaskResponse: \
                {\"thought\", \"tool_name\", \"tool_input\", \"tool_output\", \"error\", \"final_output\", \"succeeded\", \"score\"}.",
            );

            let parsed: LfmTaskResponse = self
                .send_chat_json(&self.config.executor_model, system_prompt.clone(), user_payload)
                .await?;

            final_output = parsed.final_output.clone();
            succeeded = parsed.succeeded;
            score = parsed.score;

            if let Some(ref tool_name) = parsed.tool_name {
                if !tool_name.trim().is_empty() {
                    let input_str = parsed.tool_input.as_deref().unwrap_or("{}");

                    let (tool_output, step_error) = if self.enable_script_execution {
                        match ScriptRunner::execute_tool(workspace, tool_name, input_str).await {
                            Ok(exec) if exec.exit_code == 0 => (Some(exec.stdout), None),
                            Ok(exec) => {
                                succeeded = false;
                                let out = if exec.stdout.is_empty() {
                                    None
                                } else {
                                    Some(exec.stdout)
                                };
                                let err_msg = format!(
                                    "Tool '{}' exited with code {}: {}",
                                    tool_name, exec.exit_code, exec.stderr
                                );
                                (out, Some(err_msg))
                            }
                            Err(err) => {
                                succeeded = false;
                                (None, Some(err.to_string()))
                            }
                        }
                    } else {
                        (parsed.tool_output.clone(), parsed.error.clone())
                    };

                    steps.push(TrajectoryStep {
                        turn,
                        thought: parsed.thought,
                        tool_name: Some(tool_name.clone()),
                        tool_input: parsed.tool_input,
                        tool_output,
                        error: step_error,
                    });

                    if final_output.is_some() {
                        break;
                    }
                    continue;
                }
            }

            let has_step_activity = parsed.thought.is_some()
                || parsed.tool_input.is_some()
                || parsed.tool_output.is_some()
                || parsed.error.is_some();

            if has_step_activity {
                steps.push(TrajectoryStep {
                    turn,
                    thought: parsed.thought,
                    tool_name: None,
                    tool_input: parsed.tool_input,
                    tool_output: parsed.tool_output,
                    error: parsed.error,
                });
            }

            break;
        }

        let mut trajectory = Trajectory {
            task_prompt: task_prompt.to_string(),
            steps,
            final_output,
            succeeded,
            score,
        };

        if let Some(ref evaluator) = self.evaluator {
            evaluator.apply_to_trajectory(&mut trajectory);
        }

        Ok(trajectory)
    }
}

#[async_trait]
impl ExecutionDriver for OllamaDriver {
    async fn execute_task(
        &self,
        workspace: &HarnessWorkspace,
        task_prompt: &str,
    ) -> Result<Trajectory, HarnessError> {
        if self.enable_script_execution || self.max_turns > 1 || self.evaluator.is_some() {
            return self.execute_multiturn_react(workspace, task_prompt).await;
        }

        let system_prompt = Self::build_harness_system_prompt(workspace)?;

        let mut user_payload = format!("TASK: {}\n", task_prompt);
        let attached_files = self.staged_context_files();
        if !attached_files.is_empty() {
            user_payload.push_str("\n=== ATTACHED CONTEXT FILES ===\n");
            for file in attached_files {
                user_payload.push_str(&format!(
                    "FILE [{}]:\n{}\n",
                    file.filename, file.content
                ));
            }
        }
        user_payload.push_str(
            "\nRespond strictly with JSON matching LfmTaskResponse: \
            {\"thought\", \"tool_name\", \"tool_input\", \"tool_output\", \"error\", \"final_output\", \"succeeded\", \"score\"}.",
        );

        let parsed: LfmTaskResponse = self
            .send_chat_json(&self.config.executor_model, system_prompt, user_payload)
            .await?;

        let has_step_activity = parsed.thought.is_some()
            || parsed.tool_name.is_some()
            || parsed.tool_input.is_some()
            || parsed.tool_output.is_some()
            || parsed.error.is_some();

        let steps = if has_step_activity {
            vec![TrajectoryStep {
                turn: 1,
                thought: parsed.thought,
                tool_name: parsed.tool_name,
                tool_input: parsed.tool_input,
                tool_output: parsed.tool_output,
                error: parsed.error,
            }]
        } else {
            Vec::new()
        };

        Ok(Trajectory {
            task_prompt: task_prompt.to_string(),
            steps,
            final_output: parsed.final_output,
            succeeded: parsed.succeeded,
            score: parsed.score,
        })
    }

    async fn propose_candidates(
        &self,
        workspace: &HarnessWorkspace,
        compressed_ref: &CompressedTrajectory,
        experience_log: &str,
    ) -> Result<(HarnessPatch, HarnessPatch), HarnessError> {
        let system_prompt = Self::build_harness_system_prompt(workspace)?;
        let user_prompt = format!(
            "REFERENCE TRACE:\n{}\nEXPERIENCE LOG:\n{}\n\
            Propose orthogonal Invariant and Domain harness diffs as strict JSON \
            {{\"invariant_diffs\": [...], \"domain_diffs\": [...]}}.",
            compressed_ref.summary, experience_log
        );

        let parsed: LfmProposeResponse = self
            .send_chat_json(&self.config.editor_model, system_prompt, user_prompt)
            .await?;

        let invariant_patch = HarnessPatch {
            kind: CandidateKind::Invariant,
            diffs: parsed.invariant_diffs,
        };
        let domain_patch = HarnessPatch {
            kind: CandidateKind::Domain,
            diffs: parsed.domain_diffs,
        };

        Ok((invariant_patch, domain_patch))
    }

    async fn compose(
        &self,
        workspace: &HarnessWorkspace,
        ref_trace: &CompressedTrajectory,
        invariant_patch: &HarnessPatch,
        invariant_trace: &CompressedTrajectory,
        domain_patch: &HarnessPatch,
        domain_trace: &CompressedTrajectory,
    ) -> Result<CompositionDecision, HarnessError> {
        let system_prompt = Self::build_harness_system_prompt(workspace)?;
        let user_prompt = format!(
            "REF TRACE:\n{}\nINVARIANT PATCH: {:?}\nINVARIANT TRACE:\n{}\n\
            DOMAIN PATCH: {:?}\nDOMAIN TRACE:\n{}\n\
            Synthesize winning diffs as strict JSON matching LfmComposeResponse.",
            ref_trace.summary,
            invariant_patch,
            invariant_trace.summary,
            domain_patch,
            domain_trace.summary
        );

        let parsed: LfmComposeResponse = self
            .send_chat_json(&self.config.editor_model, system_prompt, user_prompt)
            .await?;

        Ok(CompositionDecision {
            adopted_diffs: parsed.adopted_diffs,
            lesson_learned: parsed.lesson_learned,
            winning_trajectory: Trajectory {
                task_prompt: "Composed Run".to_string(),
                steps: Vec::new(),
                final_output: parsed.final_output,
                succeeded: parsed.succeeded,
                score: parsed.winning_score,
            },
        })
    }
}
