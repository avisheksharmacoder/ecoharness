use crate::state::{
    AppState, ChatSender, DetailPaneView, ExecutionStatus, Message, UiIcon,
};
use ecoharness::{
    ComponentDiff, CompositionDecision, HarnessComponent, ModelRole, MountTarget, OllamaConfig,
    OllamaModelInfo, Trajectory,
};
use lucide_icons::Icon;
use std::fs;
use tempfile::TempDir;

fn setup_test_app() -> (TempDir, TempDir, AppState) {
    let ws_dir = TempDir::new().expect("ws tempdir");
    let probe_dir = TempDir::new().expect("probe tempdir");
    let app = AppState::new(
        ws_dir.path().to_path_buf(),
        probe_dir.path().to_path_buf(),
        OllamaConfig::default(),
    )
    .expect("init AppState");
    (ws_dir, probe_dir, app)
}

// ============================================================================
// TEST 1: Left Pane Master-Detail Switching (Chat Session vs Ollama Model)
// ============================================================================
#[test]
fn test_left_pane_switching_between_chat_and_ollama_updates_detail_pane() {
    let (_ws, _probe, mut app) = setup_test_app();

    // Load discovered Ollama models into the Left Pane list
    let models = vec![
        OllamaModelInfo {
            name: "lfm2.5-350m:latest".to_string(),
            size: 367_001_600,
            digest: "sha256:lfm350m".to_string(),
            modified_at: "2026-10-22".to_string(),
        },
        OllamaModelInfo {
            name: "lfm2.5-2.6b:latest".to_string(),
            size: 1_600_000_000,
            digest: "sha256:lfm26b".to_string(),
            modified_at: "2026-10-22".to_string(),
        },
    ];
    let _ = app.update(Message::OllamaModelsLoaded(Ok(models)));
    assert_eq!(app.ollama_models.len(), 2);

    // Selecting an Ollama item switches the Left Detail Sub-Pane to OllamaModelDetail
    let _ = app.update(Message::SelectOllamaModel("lfm2.5-2.6b:latest".to_string()));
    assert_eq!(
        app.detail_view,
        DetailPaneView::OllamaModelDetail {
            model_name: "lfm2.5-2.6b:latest".to_string(),
            assigned_role: ModelRole::Executor,
        }
    );

    // Assigning the model as Editor updates the driver's OllamaConfig and the detail view
    let _ = app.update(Message::AssignModelRole(
        "lfm2.5-2.6b:latest".to_string(),
        ModelRole::Editor,
    ));
    assert_eq!(app.driver.config().editor_model, "lfm2.5-2.6b:latest");
    assert_eq!(
        app.detail_view,
        DetailPaneView::OllamaModelDetail {
            model_name: "lfm2.5-2.6b:latest".to_string(),
            assigned_role: ModelRole::Editor,
        }
    );

    // Selecting a Chat session switches the Left Detail Sub-Pane back to the 8-File HarnessInspector
    let _ = app.update(Message::SelectChatSession(0));
    assert_eq!(
        app.detail_view,
        DetailPaneView::HarnessInspector {
            session_id: 0,
            selected_component: HarnessComponent::SystemPrompt,
        }
    );
}

// ============================================================================
// TEST 2: Left Detail Sub-Pane 8-File Harness Inspector Edits & Saves to Disk
// ============================================================================
#[test]
fn test_left_detail_pane_harness_inspector_edits_and_saves_component_to_disk() {
    let (_ws, _probe, mut app) = setup_test_app();

    let _ = app.update(Message::SelectHarnessComponent(HarnessComponent::Guardrails));
    assert_eq!(
        app.detail_view,
        DetailPaneView::HarnessInspector {
            session_id: 0,
            selected_component: HarnessComponent::Guardrails,
        }
    );

    let new_guardrails = "# Updated Guardrails\nStrict schema validation enabled via GUI.";
    let _ = app.update(Message::EditHarnessComponentBuffer(
        new_guardrails.to_string(),
    ));
    let _ = app.update(Message::SaveActiveHarnessComponent);

    let on_disk = app
        .workspace
        .read_component(HarnessComponent::Guardrails)
        .expect("read guardrails from disk");
    assert_eq!(on_disk, new_guardrails);
}

// ============================================================================
// TEST 3: Right Pane "Add File" Routes to Prompt Context or 8-File Harness Slot
// ============================================================================
#[test]
fn test_right_pane_add_file_routes_to_prompt_context_or_harness_slot() {
    let (ws_dir, _probe, mut app) = setup_test_app();

    // 1. Route added file to PromptContext
    let ctx_file = ws_dir.path().join("sample_payload.json");
    fs::write(&ctx_file, "{\"order_id\": 501}").expect("write sample_payload.json");

    let _ = app.update(Message::SelectAddFileTarget(MountTarget::PromptContext));
    let _ = app.update(Message::AttachFile(ctx_file));

    assert_eq!(app.staged_files.len(), 1);
    assert_eq!(app.staged_files[0].filename, "sample_payload.json");
    assert_eq!(app.driver.staged_context_files().len(), 1);

    // 2. Route added file directly to a HarnessComponent slot (ToolGuidance)
    let custom_tool_doc = ws_dir.path().join("custom_tools.md");
    fs::write(&custom_tool_doc, "# Mounted Tool Guidance from GUI").expect("write custom_tools.md");

    let _ = app.update(Message::SelectAddFileTarget(MountTarget::HarnessSlot(
        HarnessComponent::ToolGuidance,
    )));
    let _ = app.update(Message::AttachFile(custom_tool_doc));

    assert_eq!(
        app.workspace
            .read_component(HarnessComponent::ToolGuidance)
            .unwrap(),
        "# Mounted Tool Guidance from GUI"
    );
}

// ============================================================================
// TEST 4: Right Pane Normal Chat Mode (Plan Mode OFF) Single-Pass Flow
// ============================================================================
#[test]
fn test_right_pane_normal_chat_mode_dispatches_single_pass_execution() {
    let (_ws, _probe, mut app) = setup_test_app();

    assert!(!app.plan_mode_enabled);
    let _ = app.update(Message::PromptInputChanged(
        "Extract order_id from payload".to_string(),
    ));
    let _ = app.update(Message::SendPrompt);

    assert!(
        app.prompt_input.is_empty(),
        "Prompt input box should clear upon sending"
    );
    assert_eq!(app.execution_status, ExecutionStatus::RunningSinglePass);
    assert_eq!(app.active_session().messages.len(), 1);
    assert_eq!(app.active_session().messages[0].sender, ChatSender::User);

    // Simulate async completion from the Ollama single-pass task
    let completed_traj = Trajectory {
        task_prompt: "Extract order_id from payload".to_string(),
        steps: vec![],
        final_output: Some("{\"order_id\": 501}".to_string()),
        succeeded: true,
        score: 0.90,
    };
    let _ = app.update(Message::SinglePassFinished(Ok(completed_traj)));

    assert_eq!(app.execution_status, ExecutionStatus::Idle);
    assert_eq!(app.active_session().messages.len(), 2);
    assert_eq!(
        app.active_session().messages[1].sender,
        ChatSender::Assistant
    );
    assert_eq!(
        app.active_session().messages[1].content,
        "{\"order_id\": 501}"
    );
}

// ============================================================================
// TEST 5: Right Pane Plan Mode ON Runs Propose-Probe-Compose & Records Diffs
// ============================================================================
#[test]
fn test_right_pane_plan_mode_runs_propose_probe_compose_and_records_diffs() {
    let (_ws, _probe, mut app) = setup_test_app();

    let _ = app.update(Message::TogglePlanMode(true));
    assert!(app.plan_mode_enabled);

    let _ = app.update(Message::PromptInputChanged(
        "Optimize JSON extraction tool calls".to_string(),
    ));
    let _ = app.update(Message::SendPrompt);

    assert_eq!(app.execution_status, ExecutionStatus::RunningPlanModePpc);

    let decision = CompositionDecision {
        adopted_diffs: vec![ComponentDiff {
            component: HarnessComponent::SystemPrompt,
            updated_content: "# Patched System Prompt via Plan Mode".to_string(),
            rationale: "Enforce strict JSON output".to_string(),
        }],
        lesson_learned: "Always specify output schema in SystemPrompt.".to_string(),
        winning_trajectory: Trajectory {
            task_prompt: "Optimize JSON extraction tool calls".to_string(),
            steps: vec![],
            final_output: Some("{\"status\": \"optimized\"}".to_string()),
            succeeded: true,
            score: 0.99,
        },
    };

    let _ = app.update(Message::PlanModePpcFinished(Ok(decision)));

    assert_eq!(app.execution_status, ExecutionStatus::Idle);
    let last_msg = app.active_session().messages.last().expect("assistant msg");
    assert_eq!(last_msg.sender, ChatSender::Assistant);
    assert_eq!(last_msg.adopted_diffs.len(), 1);
    assert_eq!(
        last_msg.lesson_learned.as_deref(),
        Some("Always specify output schema in SystemPrompt.")
    );
}

// ============================================================================
// TEST 6: Lucide Icons Mapping Verification
// ============================================================================
#[test]
fn test_lucide_icon_mapping_for_navigation_and_modes() {
    assert!(matches!(AppState::icon_for(UiIcon::ChatList), Icon::MessageSquare));
    assert!(matches!(AppState::icon_for(UiIcon::OllamaServer), Icon::Cpu));
    assert!(matches!(AppState::icon_for(UiIcon::AddFile), Icon::Paperclip));
    assert!(matches!(AppState::icon_for(UiIcon::PlanMode), Icon::GitBranch));
}

// ============================================================================
// PHASE 4B TDD SUITE (crates/ecoharness-gui): Multi-line Editor, Live Events & Persistence
// ============================================================================
use ecoharness::PpcStageEvent;
use iced::widget::text_editor::{Action, Edit};
use std::sync::Arc;

// ============================================================================
// TEST 7: Multi-Line text_editor::Action Updates Inspector & Saves to Disk
// ============================================================================
#[test]
fn test_gui_multiline_text_editor_action_updates_and_saves_component() {
    let (_ws, _probe, mut app) = setup_test_app();

    let _ = app.update(Message::SelectHarnessComponent(
        HarnessComponent::ToolGuidance,
    ));

    let multiline_md = "# Tool Guidance v2\n- Line 1: Use integer amount_cents\n- Line 2: Use uppercase currency";
    let _ = app.update(Message::ComponentEditorAction(Action::SelectAll));
    let _ = app.update(Message::ComponentEditorAction(Action::Edit(Edit::Paste(
        Arc::new(multiline_md.to_string()),
    ))));

    assert_eq!(app.component_editor_buffer.trim(), multiline_md);
    assert_eq!(app.component_editor_content.text().trim(), multiline_md);

    let _ = app.update(Message::SaveActiveHarnessComponent);
    let saved_on_disk = app
        .workspace
        .read_component(HarnessComponent::ToolGuidance)
        .expect("read saved multiline component");
    assert_eq!(saved_on_disk.trim(), multiline_md);
}

// ============================================================================
// TEST 8: Live PPC Stage Progress Events Update GUI History & Status
// ============================================================================
#[test]
fn test_gui_ppc_stage_progress_events_update_status_and_history() {
    let (_ws, _probe, mut app) = setup_test_app();

    let _ = app.update(Message::PpcStageProgressed(PpcStageEvent::ReferenceStarted));
    let _ = app.update(Message::PpcStageProgressed(
        PpcStageEvent::ReferenceCompleted {
            score: 0.25,
            succeeded: false,
        },
    ));
    let _ = app.update(Message::PpcStageProgressed(
        PpcStageEvent::CandidatesProposed {
            invariant_diff_count: 1,
            domain_diff_count: 1,
        },
    ));

    assert_eq!(app.ppc_stage_history.len(), 3);
    assert_eq!(app.execution_status, ExecutionStatus::RunningPlanModePpc);
}

// ============================================================================
// TEST 9: On-Disk Session Persistence Saves & Reloads Chat History
// ============================================================================
#[test]
fn test_gui_sessions_persist_to_and_reload_from_disk() {
    let (ws_dir, probe_dir, mut app) = setup_test_app();

    let _ = app.update(Message::PromptInputChanged(
        "Persist this prompt across restart".to_string(),
    ));
    let _ = app.update(Message::SendPrompt);

    let sessions_file = app.save_sessions().expect("save sessions.json");
    assert!(sessions_file.exists());

    // Instantiate a fresh AppState pointing at the same workspace directory and load sessions
    let mut reloaded_app = AppState::new(
        ws_dir.path().to_path_buf(),
        probe_dir.path().to_path_buf(),
        OllamaConfig::default(),
    )
    .expect("re-init AppState");

    reloaded_app
        .load_sessions()
        .expect("load persisted sessions");
    assert_eq!(reloaded_app.active_session().messages.len(), 1);
    assert_eq!(
        reloaded_app.active_session().messages[0].content,
        "Persist this prompt across restart"
    );
}

// ============================================================================
// TEST 10: GUI Snapshot Save & Rollback Restores Inspector Buffer
// ============================================================================
#[test]
fn test_gui_snapshot_save_and_rollback_restores_inspector_buffer() {
    let (_ws, _probe, mut app) = setup_test_app();

    let _ = app.update(Message::SelectHarnessComponent(
        HarnessComponent::SystemPrompt,
    ));
    let _ = app.update(Message::EditHarnessComponentBuffer(
        "# Stable Prompt v1".to_string(),
    ));
    let _ = app.update(Message::SaveActiveHarnessComponent);
    let _ = app.update(Message::SaveSnapshotVersion("v1".to_string()));

    assert!(app.saved_snapshots.contains(&"v1".to_string()));

    // Overwrite with broken prompt
    let _ = app.update(Message::EditHarnessComponentBuffer(
        "# Broken Prompt v2".to_string(),
    ));
    let _ = app.update(Message::SaveActiveHarnessComponent);
    assert_eq!(
        app.workspace
            .read_component(HarnessComponent::SystemPrompt)
            .unwrap(),
        "# Broken Prompt v2"
    );

    // Roll back to v1 via GUI message
    let _ = app.update(Message::RollbackSnapshotVersion("v1".to_string()));
    assert_eq!(app.component_editor_buffer, "# Stable Prompt v1");
    assert_eq!(
        app.workspace
            .read_component(HarnessComponent::SystemPrompt)
            .unwrap(),
        "# Stable Prompt v1"
    );
}

// ============================================================================
// PHASE 5 TDD SUITE (crates/harness2-gui): Startup Restore, Script Toggle & Live PPC
// ============================================================================

// ============================================================================
// TEST 11: Startup Restore Loads Persisted Sessions & Enables Multi-Turn Scripts
// ============================================================================
#[test]
fn test_gui_startup_restore_loads_sessions_and_enables_multiturn_scripts() {
    let (ws_dir, probe_dir, mut initial_app) = setup_test_app();

    let _ = initial_app.update(Message::PromptInputChanged(
        "Saved before app close".to_string(),
    ));
    let _ = initial_app.update(Message::SendPrompt);
    initial_app.save_sessions().expect("save sessions");

    let restored_app = AppState::new_with_startup_restore(
        ws_dir.path().to_path_buf(),
        probe_dir.path().to_path_buf(),
        OllamaConfig::default(),
    )
    .expect("new_with_startup_restore");

    assert_eq!(
        restored_app.active_session().messages.len(),
        1,
        "new_with_startup_restore must auto-load sessions.json on launch"
    );
    assert_eq!(
        restored_app.active_session().messages[0].content,
        "Saved before app close"
    );
    assert!(restored_app.script_execution_enabled);
    assert_eq!(restored_app.driver.max_turns(), 3);
    assert!(restored_app.driver.script_execution_enabled());
}

// ============================================================================
// TEST 12: GUI Toggles Multi-Turn ScriptRunner & Updates Temperature
// ============================================================================
#[test]
fn test_gui_toggle_script_execution_and_temperature_updates_driver() {
    let (_ws, _probe, mut app) = setup_test_app();

    let _ = app.update(Message::ToggleScriptExecution(true));
    assert!(app.script_execution_enabled);
    assert!(app.driver.script_execution_enabled());
    assert_eq!(app.driver.max_turns(), 3);

    let _ = app.update(Message::UpdateTemperature(0.45));
    assert!((app.driver.config().temperature - 0.45).abs() < f32::EPSILON);

    let summary = app.inspector_summary();
    assert!(
        summary.contains("0.45") && summary.contains("Scripts: ON"),
        "Inspector summary must report temperature and script execution status, got: {}",
        summary
    );
}

// ============================================================================
// TEST 13: ApplyLivePpcRun Records All 5 Stage Events & Updates Inspector Summary
// ============================================================================
#[test]
fn test_gui_live_ppc_batch_progress_populates_stage_history_and_experience() {
    let (_ws, _probe, mut app) = setup_test_app();

    let stages = vec![
        PpcStageEvent::ReferenceStarted,
        PpcStageEvent::ReferenceCompleted {
            score: 0.25,
            succeeded: false,
        },
        PpcStageEvent::CandidatesProposed {
            invariant_diff_count: 1,
            domain_diff_count: 1,
        },
        PpcStageEvent::ProbesCompleted {
            invariant_score: 0.98,
            domain_score: 0.40,
        },
        PpcStageEvent::Composed {
            adopted_diff_count: 1,
            winning_score: 0.98,
        },
    ];

    let decision = CompositionDecision {
        adopted_diffs: vec![ComponentDiff {
            component: HarnessComponent::ToolGuidance,
            updated_content: "# Patched v2 ToolGuidance".to_string(),
            rationale: "Use integer amount_cents".to_string(),
        }],
        lesson_learned: "Normalize currency strings to integer cents.".to_string(),
        winning_trajectory: Trajectory {
            task_prompt: "Charge invoice".to_string(),
            steps: vec![],
            final_output: Some("{\"amount_cents\": 12550}".to_string()),
            succeeded: true,
            score: 0.98,
        },
    };

    let _ = app.update(Message::ApplyLivePpcRun(stages, Ok(decision)));

    assert_eq!(app.ppc_stage_history.len(), 5);
    assert_eq!(app.execution_status, ExecutionStatus::Idle);
    assert_eq!(
        app.active_session()
            .messages
            .last()
            .unwrap()
            .lesson_learned
            .as_deref(),
        Some("Normalize currency strings to integer cents.")
    );
}

