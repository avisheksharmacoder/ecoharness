use ecoharness::{
    ComponentDiff, CompositionDecision, FileMounter, HarnessComponent, HarnessEngine,
    HarnessWorkspace, ModelRole, MountOutcome, MountTarget, OllamaConfig, OllamaDriver,
    OllamaModelInfo, PpcStageEvent, StagedContextFile, Trajectory,
};
use iced::widget::text_editor;
use iced::Task;
use lucide_icons::Icon;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SESSIONS_FILENAME: &str = "sessions.json";

/// Identifies who sent a message in the Right Pane chat stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatSender {
    User,
    Assistant,
    SystemNotice,
}

/// A single message item rendered in the Right Pane conversation view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub sender: ChatSender,
    pub content: String,
    pub score: Option<f32>,
    /// Populated when Plan Mode (Propose-Probe-Compose) produces a composed harness diff.
    pub adopted_diffs: Vec<ComponentDiff>,
    pub lesson_learned: Option<String>,
}

/// A user chat session listed in the Left Pane Master List.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: usize,
    pub title: String,
    pub messages: Vec<ChatMessage>,
}

/// Controls what is displayed in the Left Pane's contextual bottom Detail Sub-Pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailPaneView {
    /// Active when a Chat Session is selected: inspects/edits the 8 Harness² files.
    HarnessInspector {
        session_id: usize,
        selected_component: HarnessComponent,
    },
    /// Active when an Ollama model item is selected: configures endpoint, temperature, and role.
    OllamaModelDetail {
        model_name: String,
        assigned_role: ModelRole,
    },
}

/// Tracks whether the Right Pane is idle, running a normal chat turn, or executing Plan Mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStatus {
    Idle,
    RunningSinglePass,
    RunningPlanModePpc,
    Error(String),
}

/// All semantic UI actions mapped to Lucide vector icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiIcon {
    ChatList,
    OllamaServer,
    HarnessFiles,
    AddFile,
    PlanMode,
    SendPrompt,
    RefreshModels,
    SaveComponent,
}

/// Complete Elm `Message` contract for the 2-pane Iced application.
#[derive(Debug, Clone)]
pub enum Message {
    // --- Left Pane: Master Lists & Contextual Detail Sub-Pane ---
    SelectChatSession(usize),
    CreateNewChatSession,
    SelectOllamaModel(String),
    RefreshOllamaModels,
    OllamaModelsLoaded(Result<Vec<OllamaModelInfo>, String>),
    AssignModelRole(String, ModelRole),
    UpdateOllamaEndpoint(String),
    UpdateTemperature(f32),
    ToggleEnforceJsonSchema(bool),
    ToggleScriptExecution(bool),

    // --- Left Detail Sub-Pane: 8-File Harness Inspector & Multi-line Editor ---
    SelectHarnessComponent(HarnessComponent),
    EditHarnessComponentBuffer(String),
    ComponentEditorAction(text_editor::Action),
    SaveActiveHarnessComponent,
    SaveSnapshotVersion(String),
    RollbackSnapshotVersion(String),

    // --- Right Pane: Chat Input, Add File & Plan Mode ---
    PromptInputChanged(String),
    TogglePlanMode(bool),
    SelectAddFileTarget(MountTarget),
    OpenNativeFileDialog,
    AttachFile(PathBuf),
    ClearStagedFiles,
    SendPrompt,

    // --- Async SDK Progress & Completion Callbacks ---
    PpcStageProgressed(PpcStageEvent),
    ApplyLivePpcRun(Vec<PpcStageEvent>, Result<CompositionDecision, String>),
    SinglePassFinished(Result<Trajectory, String>),
    PlanModePpcFinished(Result<CompositionDecision, String>),
}

/// Complete state of the 2-pane `harness2-gui` workbench.
pub struct AppState {
    pub workspace: HarnessWorkspace,
    pub probe_root: PathBuf,
    pub driver: OllamaDriver,

    // Left Pane: Top Master Lists
    pub sessions: Vec<ChatSession>,
    pub active_session_id: usize,
    pub ollama_models: Vec<OllamaModelInfo>,
    pub selected_ollama_model: Option<String>,

    // Left Pane: Bottom Contextual Detail Sub-Pane
    pub detail_view: DetailPaneView,
    pub component_editor_buffer: String,
    pub component_editor_content: text_editor::Content,
    pub saved_snapshots: Vec<String>,
    pub experience_preview: String,
    pub script_execution_enabled: bool,
    editor_select_all_pending: bool,

    // Right Pane: Chat, Add File & Plan Mode
    pub prompt_input: String,
    pub plan_mode_enabled: bool,
    pub add_file_target: MountTarget,
    pub staged_files: Vec<StagedContextFile>,
    pub execution_status: ExecutionStatus,
    pub ppc_stage_history: Vec<PpcStageEvent>,
}

impl AppState {
    /// Initializes the GUI state against a `HarnessWorkspace` directory and probe directory.
    pub fn new(
        workspace_root: PathBuf,
        probe_root: PathBuf,
        config: OllamaConfig,
    ) -> Result<Self, String> {
        let workspace = if workspace_root.join("EXPERIENCE.md").exists() {
            HarnessWorkspace::load(&workspace_root).map_err(|e| e.to_string())?
        } else {
            HarnessWorkspace::init(&workspace_root).map_err(|e| e.to_string())?
        };

        let initial_component = HarnessComponent::SystemPrompt;
        let component_editor_buffer = workspace
            .read_component(initial_component)
            .map_err(|e| e.to_string())?;
        let component_editor_content = text_editor::Content::with_text(&component_editor_buffer);
        let experience_preview = workspace.read_experience().map_err(|e| e.to_string())?;

        let driver = OllamaDriver::new(config);
        let default_session = ChatSession {
            id: 0,
            title: "Session 1".to_string(),
            messages: Vec::new(),
        };

        Ok(Self {
            workspace,
            probe_root,
            driver,
            sessions: vec![default_session],
            active_session_id: 0,
            ollama_models: Vec::new(),
            selected_ollama_model: None,
            detail_view: DetailPaneView::HarnessInspector {
                session_id: 0,
                selected_component: initial_component,
            },
            component_editor_buffer,
            component_editor_content,
            saved_snapshots: Vec::new(),
            experience_preview,
            script_execution_enabled: false,
            editor_select_all_pending: false,
            prompt_input: String::new(),
            plan_mode_enabled: false,
            add_file_target: MountTarget::PromptContext,
            staged_files: Vec::new(),
            execution_status: ExecutionStatus::Idle,
            ppc_stage_history: Vec::new(),
        })
    }

    /// Initializes `AppState`, automatically loads `<workspace_root>/sessions.json` if present,
    /// and configures the `OllamaDriver` with multi-turn script, subagent, and plugin execution enabled.
    pub fn new_with_startup_restore(
        workspace_root: PathBuf,
        probe_root: PathBuf,
        config: OllamaConfig,
    ) -> Result<Self, String> {
        let mut state = Self::new(workspace_root, probe_root, config)?;
        state.script_execution_enabled = true;
        state.driver = state
            .driver
            .with_max_turns(3)
            .with_script_execution(true)
            .with_subagents(true)
            .with_plugins(true);
        state.load_sessions()?;
        Ok(state)
    }

    /// Returns a formatted diagnostic summary of the Left Detail Sub-Pane (active file,
    /// `EXPERIENCE.md` preview, temperature, and multi-turn script status).
    pub fn inspector_summary(&self) -> String {
        let scripts_flag = if self.script_execution_enabled {
            "Scripts: ON"
        } else {
            "Scripts: OFF"
        };
        let exp_first_line = self
            .experience_preview
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("No experience entries");
        format!(
            "Temp: {:.2} | {} | MaxTurns: {} | Exp: {}",
            self.driver.config().temperature,
            scripts_flag,
            self.driver.max_turns(),
            exp_first_line
        )
    }

    fn sessions_file_path(&self) -> PathBuf {
        self.workspace.root().join(SESSIONS_FILENAME)
    }

    /// Persists all `ChatSession`s to `<workspace_root>/sessions.json`.
    pub fn save_sessions(&self) -> Result<PathBuf, String> {
        let path = self.sessions_file_path();
        let json = serde_json::to_string_pretty(&self.sessions).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// Loads `ChatSession`s from `<workspace_root>/sessions.json` if present.
    pub fn load_sessions(&mut self) -> Result<(), String> {
        let path = self.sessions_file_path();
        if !path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let loaded: Vec<ChatSession> = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if !loaded.is_empty() {
            self.active_session_id = loaded[0].id;
            self.sessions = loaded;
        }
        Ok(())
    }

    fn sync_editor_from_string(&mut self, content: String) {
        self.component_editor_content = text_editor::Content::with_text(&content);
        self.component_editor_buffer = content;
        self.editor_select_all_pending = false;
    }

    /// Returns a reference to the currently active `ChatSession`.
    pub fn active_session(&self) -> &ChatSession {
        self.sessions
            .iter()
            .find(|s| s.id == self.active_session_id)
            .unwrap_or(&self.sessions[0])
    }

    fn active_session_mut(&mut self) -> &mut ChatSession {
        let active_id = self.active_session_id;
        if let Some(pos) = self.sessions.iter().position(|s| s.id == active_id) {
            &mut self.sessions[pos]
        } else {
            &mut self.sessions[0]
        }
    }

    /// Maps semantic UI actions to `lucide_icons::Icon` variants.
    pub fn icon_for(action: UiIcon) -> Icon {
        match action {
            UiIcon::ChatList => Icon::MessageSquare,
            UiIcon::OllamaServer => Icon::Cpu,
            UiIcon::HarnessFiles => Icon::FileText,
            UiIcon::AddFile => Icon::Paperclip,
            UiIcon::PlanMode => Icon::GitBranch,
            UiIcon::SendPrompt => Icon::Send,
            UiIcon::RefreshModels => Icon::RefreshCw,
            UiIcon::SaveComponent => Icon::Save,
        }
    }

    fn infer_model_role(&self, model_name: &str) -> ModelRole {
        let cfg = self.driver.config();
        if cfg.executor_model == model_name && cfg.editor_model == model_name {
            ModelRole::Both
        } else if cfg.editor_model == model_name {
            ModelRole::Editor
        } else {
            ModelRole::Executor
        }
    }

    /// Pure Elm state transition function tested headlessly before rendering.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectChatSession(session_id) => {
                if self.sessions.iter().any(|s| s.id == session_id) {
                    self.active_session_id = session_id;
                }
                let selected_component = match self.detail_view {
                    DetailPaneView::HarnessInspector {
                        selected_component, ..
                    } => selected_component,
                    DetailPaneView::OllamaModelDetail { .. } => HarnessComponent::SystemPrompt,
                };
                self.detail_view = DetailPaneView::HarnessInspector {
                    session_id: self.active_session_id,
                    selected_component,
                };
                if let Ok(content) = self.workspace.read_component(selected_component) {
                    self.sync_editor_from_string(content);
                }
                Task::none()
            }

            Message::CreateNewChatSession => {
                let next_id = self.sessions.iter().map(|s| s.id).max().unwrap_or(0) + 1;
                self.sessions.push(ChatSession {
                    id: next_id,
                    title: format!("Session {}", next_id + 1),
                    messages: Vec::new(),
                });
                self.active_session_id = next_id;
                self.detail_view = DetailPaneView::HarnessInspector {
                    session_id: next_id,
                    selected_component: HarnessComponent::SystemPrompt,
                };
                if let Ok(content) = self.workspace.read_component(HarnessComponent::SystemPrompt) {
                    self.sync_editor_from_string(content);
                }
                let _ = self.save_sessions();
                Task::none()
            }

            Message::SelectOllamaModel(model_name) => {
                let assigned_role = self.infer_model_role(&model_name);
                self.selected_ollama_model = Some(model_name.clone());
                self.detail_view = DetailPaneView::OllamaModelDetail {
                    model_name,
                    assigned_role,
                };
                Task::none()
            }

            Message::RefreshOllamaModels => {
                let driver = self.driver.clone();
                Task::perform(
                    async move { driver.list_models().await.map_err(|e| e.to_string()) },
                    Message::OllamaModelsLoaded,
                )
            }

            Message::OllamaModelsLoaded(result) => {
                match result {
                    Ok(models) => {
                        self.ollama_models = models;
                    }
                    Err(err) => {
                        self.execution_status = ExecutionStatus::Error(err);
                    }
                }
                Task::none()
            }

            Message::AssignModelRole(model_name, role) => {
                let mut cfg = self.driver.config().clone();
                match role {
                    ModelRole::Executor => cfg.executor_model = model_name.clone(),
                    ModelRole::Editor => cfg.editor_model = model_name.clone(),
                    ModelRole::Both => {
                        cfg.executor_model = model_name.clone();
                        cfg.editor_model = model_name.clone();
                    }
                }
                self.driver.set_config(cfg);
                self.detail_view = DetailPaneView::OllamaModelDetail {
                    model_name,
                    assigned_role: role,
                };
                Task::none()
            }

            Message::UpdateOllamaEndpoint(url) => {
                let mut cfg = self.driver.config().clone();
                cfg.base_url = url;
                self.driver.set_config(cfg);
                Task::none()
            }

            Message::UpdateTemperature(temp) => {
                let mut cfg = self.driver.config().clone();
                cfg.temperature = temp.clamp(0.0, 2.0);
                self.driver.set_config(cfg);
                Task::none()
            }

            Message::ToggleEnforceJsonSchema(enforce) => {
                let mut cfg = self.driver.config().clone();
                cfg.enforce_json_schema = enforce;
                self.driver.set_config(cfg);
                Task::none()
            }

            Message::ToggleScriptExecution(enabled) => {
                self.script_execution_enabled = enabled;
                let turns = if enabled { 3 } else { 1 };
                self.driver = self
                    .driver
                    .clone()
                    .with_max_turns(turns)
                    .with_script_execution(enabled)
                    .with_subagents(enabled)
                    .with_plugins(enabled);
                Task::none()
            }

            Message::SelectHarnessComponent(component) => {
                self.detail_view = DetailPaneView::HarnessInspector {
                    session_id: self.active_session_id,
                    selected_component: component,
                };
                if let Ok(content) = self.workspace.read_component(component) {
                    self.sync_editor_from_string(content);
                }
                Task::none()
            }

            Message::EditHarnessComponentBuffer(updated) => {
                self.sync_editor_from_string(updated);
                Task::none()
            }

            Message::ComponentEditorAction(action) => {
                if matches!(action, text_editor::Action::SelectAll) {
                    self.editor_select_all_pending = true;
                    self.component_editor_content.perform(action);
                    return Task::none();
                }

                if self.editor_select_all_pending {
                    if let text_editor::Action::Edit(text_editor::Edit::Paste(ref pasted)) = action
                    {
                        let replaced = pasted.as_ref().clone();
                        self.sync_editor_from_string(replaced);
                        return Task::none();
                    }
                    self.editor_select_all_pending = false;
                }

                self.component_editor_content.perform(action);
                self.component_editor_buffer = self.component_editor_content.text();
                Task::none()
            }

            Message::SaveActiveHarnessComponent => {
                if let DetailPaneView::HarnessInspector {
                    selected_component, ..
                } = self.detail_view
                {
                    if let Err(err) = self
                        .workspace
                        .write_component(selected_component, &self.component_editor_buffer)
                    {
                        self.execution_status = ExecutionStatus::Error(err.to_string());
                    }
                }
                Task::none()
            }

            Message::SaveSnapshotVersion(tag) => {
                match self.workspace.save_versioned_snapshot(&tag) {
                    Ok(_) => {
                        if !self.saved_snapshots.contains(&tag) {
                            self.saved_snapshots.push(tag);
                        }
                    }
                    Err(err) => {
                        self.execution_status = ExecutionStatus::Error(err.to_string());
                    }
                }
                Task::none()
            }

            Message::RollbackSnapshotVersion(tag) => {
                match self.workspace.rollback_to_snapshot(&tag) {
                    Ok(()) => {
                        if let DetailPaneView::HarnessInspector {
                            selected_component, ..
                        } = self.detail_view
                        {
                            if let Ok(content) = self.workspace.read_component(selected_component) {
                                self.sync_editor_from_string(content);
                            }
                        }
                    }
                    Err(err) => {
                        self.execution_status = ExecutionStatus::Error(err.to_string());
                    }
                }
                Task::none()
            }

            Message::PromptInputChanged(text) => {
                self.prompt_input = text;
                Task::none()
            }

            Message::TogglePlanMode(enabled) => {
                self.plan_mode_enabled = enabled;
                Task::none()
            }

            Message::SelectAddFileTarget(target) => {
                self.add_file_target = target;
                Task::none()
            }

            Message::OpenNativeFileDialog => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .pick_file()
                        .await
                        .map(|handle| handle.path().to_path_buf())
                },
                |maybe_path| match maybe_path {
                    Some(path) => Message::AttachFile(path),
                    None => Message::PromptInputChanged(String::new()),
                },
            ),

            Message::AttachFile(path) => {
                match FileMounter::mount_file(
                    &self.workspace,
                    &self.driver,
                    &path,
                    self.add_file_target,
                ) {
                    Ok(MountOutcome::StagedInContext(_)) => {
                        self.staged_files = self.driver.staged_context_files();
                    }
                    Ok(MountOutcome::MountedToComponent(comp)) => {
                        if let DetailPaneView::HarnessInspector {
                            selected_component, ..
                        } = self.detail_view
                        {
                            if selected_component == comp {
                                if let Ok(content) = self.workspace.read_component(comp) {
                                    self.sync_editor_from_string(content);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        self.execution_status = ExecutionStatus::Error(err.to_string());
                    }
                }
                Task::none()
            }

            Message::ClearStagedFiles => {
                self.driver.clear_staged_context();
                self.staged_files.clear();
                Task::none()
            }

            Message::SendPrompt => {
                let prompt = self.prompt_input.trim().to_string();
                if prompt.is_empty() {
                    return Task::none();
                }
                self.prompt_input.clear();

                self.active_session_mut().messages.push(ChatMessage {
                    sender: ChatSender::User,
                    content: prompt.clone(),
                    score: None,
                    adopted_diffs: Vec::new(),
                    lesson_learned: None,
                });
                let _ = self.save_sessions();

                let ws = self.workspace.clone();
                let driver = self.driver.clone();

                if self.plan_mode_enabled {
                    self.execution_status = ExecutionStatus::RunningPlanModePpc;
                    self.ppc_stage_history.clear();
                    let probe_root = self.probe_root.clone();
                    Task::perform(
                        async move {
                            let engine =
                                HarnessEngine::new(driver, 1200).with_cleanup_sandboxes(true);
                            let mut stages = Vec::new();
                            let res = engine
                                .run_propose_probe_compose_with_progress(
                                    &ws,
                                    &probe_root,
                                    &prompt,
                                    |evt| stages.push(evt),
                                )
                                .await
                                .map_err(|e| e.to_string());
                            (stages, res)
                        },
                        |(stages, res)| Message::ApplyLivePpcRun(stages, res),
                    )
                } else {
                    self.execution_status = ExecutionStatus::RunningSinglePass;
                    Task::perform(
                        async move {
                            let engine = HarnessEngine::new(driver, 1200);
                            engine
                                .run_single_pass(&ws, &prompt)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Message::SinglePassFinished,
                    )
                }
            }

            Message::PpcStageProgressed(event) => {
                let is_terminal = matches!(event, PpcStageEvent::Composed { .. });
                self.ppc_stage_history.push(event);
                if is_terminal {
                    self.execution_status = ExecutionStatus::Idle;
                } else {
                    self.execution_status = ExecutionStatus::RunningPlanModePpc;
                }
                Task::none()
            }

            Message::ApplyLivePpcRun(stages, result) => {
                self.ppc_stage_history = stages;
                self.update(Message::PlanModePpcFinished(result))
            }

            Message::SinglePassFinished(result) => {
                match result {
                    Ok(trajectory) => {
                        self.execution_status = ExecutionStatus::Idle;
                        let content = trajectory
                            .final_output
                            .unwrap_or_else(|| "(Completed with no final_output)".to_string());
                        self.active_session_mut().messages.push(ChatMessage {
                            sender: ChatSender::Assistant,
                            content,
                            score: Some(trajectory.score),
                            adopted_diffs: Vec::new(),
                            lesson_learned: None,
                        });
                    }
                    Err(err) => {
                        self.execution_status = ExecutionStatus::Error(err.clone());
                        self.active_session_mut().messages.push(ChatMessage {
                            sender: ChatSender::SystemNotice,
                            content: format!("Execution error: {}", err),
                            score: None,
                            adopted_diffs: Vec::new(),
                            lesson_learned: None,
                        });
                    }
                }
                let _ = self.save_sessions();
                Task::none()
            }

            Message::PlanModePpcFinished(result) => {
                match result {
                    Ok(decision) => {
                        self.execution_status = ExecutionStatus::Idle;
                        if let Ok(exp) = self.workspace.read_experience() {
                            self.experience_preview = exp;
                        }
                        if let DetailPaneView::HarnessInspector {
                            selected_component, ..
                        } = self.detail_view
                        {
                            if let Ok(content) = self.workspace.read_component(selected_component) {
                                self.sync_editor_from_string(content);
                            }
                        }

                        let content = decision
                            .winning_trajectory
                            .final_output
                            .unwrap_or_else(|| "(Plan Mode completed)".to_string());

                        self.active_session_mut().messages.push(ChatMessage {
                            sender: ChatSender::Assistant,
                            content,
                            score: Some(decision.winning_trajectory.score),
                            adopted_diffs: decision.adopted_diffs,
                            lesson_learned: Some(decision.lesson_learned),
                        });
                    }
                    Err(err) => {
                        self.execution_status = ExecutionStatus::Error(err.clone());
                        self.active_session_mut().messages.push(ChatMessage {
                            sender: ChatSender::SystemNotice,
                            content: format!("Plan Mode PPC error: {}", err),
                            score: None,
                            adopted_diffs: Vec::new(),
                            lesson_learned: None,
                        });
                    }
                }
                let _ = self.save_sessions();
                Task::none()
            }
        }
    }
}
