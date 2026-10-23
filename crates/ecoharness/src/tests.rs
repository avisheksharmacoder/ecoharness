use crate::{
    CandidateKind, ComponentDiff, CompressedTrajectory, CompositionDecision, ExecutionDriver,
    HarnessComponent, HarnessEngine, HarnessError, HarnessPatch, HarnessWorkspace, Trajectory,
    TrajectoryCompressor, TrajectoryStep,
};
use async_trait::async_trait;
use tempfile::TempDir;

// ============================================================================
// TEST 1: 8-Component Workspace & EXPERIENCE.md Initialization
// ============================================================================
#[test]
fn test_workspace_init_creates_all_8_components_and_experience_file() {
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init workspace");

    for component in HarnessComponent::all() {
        let full_path = temp.path().join(component.relative_path());
        assert!(
            full_path.exists(),
            "Expected component file {:?} at {:?}",
            component,
            full_path
        );
        let content = ws.read_component(component).expect("read component");
        assert!(
            !content.is_empty(),
            "Component {:?} should be initialized with non-empty default content",
            component
        );
    }

    let exp_path = temp.path().join("EXPERIENCE.md");
    assert!(exp_path.exists(), "EXPERIENCE.md must exist on init");
    let _loaded = HarnessWorkspace::load(temp.path()).expect("reload initialized workspace");
}

// ============================================================================
// TEST 2: Sandbox Cloning & Mutation Isolation
// ============================================================================
#[test]
fn test_workspace_sandbox_isolation_preserves_base_on_mutation() {
    let base_dir = TempDir::new().expect("base dir");
    let sandbox_dir = TempDir::new().expect("sandbox dir");

    let base_ws = HarnessWorkspace::init(base_dir.path()).expect("init base");
    let original_snapshot = base_ws.snapshot().expect("snapshot before probe");

    let sandbox_ws = base_ws
        .clone_to_sandbox(sandbox_dir.path())
        .expect("clone to sandbox");

    sandbox_ws
        .write_component(
            HarnessComponent::SystemPrompt,
            "You are a mutated probe candidate.",
        )
        .expect("mutate sandbox system prompt");
    sandbox_ws
        .write_component(
            HarnessComponent::ToolGuidance,
            "Always format tool args as strict JSON.",
        )
        .expect("mutate sandbox tool guidance");

    let post_probe_base_snapshot = base_ws.snapshot().expect("snapshot after probe");
    assert_eq!(
        original_snapshot, post_probe_base_snapshot,
        "Mutating a probe sandbox must NEVER alter the base workspace"
    );
    assert_eq!(
        sandbox_ws
            .read_component(HarnessComponent::ToolGuidance)
            .unwrap(),
        "Always format tool args as strict JSON."
    );
}

// ============================================================================
// TEST 3: Transactional Patch Application & Subagents JSON Validation
// ============================================================================
#[test]
fn test_workspace_patch_rejects_invalid_subagents_json_and_rolls_back() {
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init");
    let before = ws.snapshot().expect("snapshot before invalid patch");

    let bad_patch = HarnessPatch {
        kind: CandidateKind::Domain,
        diffs: vec![
            ComponentDiff {
                component: HarnessComponent::Memory,
                updated_content: "Valid memory update that should be rolled back".to_string(),
                rationale: "Add domain memory".to_string(),
            },
            ComponentDiff {
                component: HarnessComponent::Subagents,
                updated_content: "{ malformed_json: [1, 2, ".to_string(),
                rationale: "Broken subagent config".to_string(),
            },
        ],
    };

    let res = ws.apply_patch(&bad_patch);
    assert!(
        matches!(
            res,
            Err(HarnessError::InvalidComponentFormat {
                component: HarnessComponent::Subagents,
                ..
            })
        ),
        "Expected InvalidComponentFormat for Subagents, got: {:?}",
        res
    );

    let after = ws.snapshot().expect("snapshot after failed patch");
    assert_eq!(
        before, after,
        "Failed patch application must atomically roll back earlier diffs in the same patch"
    );
}

// ============================================================================
// TEST 4: LFM2.5-350M Trajectory Compressor Enforces Budget & Preserves Errors
// ============================================================================
#[test]
fn test_trajectory_compressor_enforces_strict_budget_for_lfm_350m() {
    let mut steps = Vec::new();
    for turn in 1..=20 {
        let is_error_turn = turn == 14;
        steps.push(TrajectoryStep {
            turn,
            thought: Some(format!("Verbose reasoning step {} {}", turn, "x".repeat(400))),
            tool_name: Some("json_extractor".to_string()),
            tool_input: Some(format!("{{\"chunk\": {}}}", turn)),
            tool_output: if is_error_turn {
                None
            } else {
                Some("VERBOSE_OK_OUTPUT_".repeat(40))
            },
            error: if is_error_turn {
                Some("CRITICAL_TOOL_ERR: missing required field 'user_id'".to_string())
            } else {
                None
            },
        });
    }

    let raw_trace = Trajectory {
        task_prompt: "Extract user_id from payload".to_string(),
        steps,
        final_output: Some("Failed extraction".to_string()),
        succeeded: false,
        score: 0.1,
    };

    let max_chars = 1200;
    let compressed =
        TrajectoryCompressor::compress(&raw_trace, max_chars).expect("compress trajectory");

    assert!(
        compressed.char_len <= max_chars,
        "Compressed char_len ({}) exceeded budget ({})",
        compressed.char_len,
        max_chars
    );
    assert_eq!(compressed.char_len, compressed.summary.chars().count());
    assert!(
        compressed.summary.contains("Extract user_id from payload"),
        "Compressed summary must retain the task prompt"
    );
    assert!(
        compressed
            .summary
            .contains("CRITICAL_TOOL_ERR: missing required field 'user_id'"),
        "Compressed summary must prioritize and retain tool execution errors"
    );
}

// ============================================================================
// MockDriver for Deterministic Propose-Probe-Compose Engine Tests
// ============================================================================
struct ScriptedMockDriver {
    both_regress: bool,
}

#[async_trait]
impl ExecutionDriver for ScriptedMockDriver {
    async fn execute_task(
        &self,
        workspace: &HarnessWorkspace,
        task_prompt: &str,
    ) -> Result<Trajectory, HarnessError> {
        let tool_guidance = workspace.read_component(HarnessComponent::ToolGuidance)?;
        let memory = workspace.read_component(HarnessComponent::Memory)?;

        if self.both_regress {
            // Base run scores 0.6; mutated candidates score lower (0.2)
            let is_mutated =
                tool_guidance.contains("PATCHED") || memory.contains("PATCHED");
            return Ok(Trajectory {
                task_prompt: task_prompt.to_string(),
                steps: vec![],
                final_output: Some("output".to_string()),
                succeeded: !is_mutated,
                score: if is_mutated { 0.2 } else { 0.6 },
            });
        }

        // Normal test case: if the Invariant patch is mounted on disk, the probe succeeds (0.95)
        if tool_guidance.contains("STRICT_USER_ID_SCHEMA") {
            Ok(Trajectory {
                task_prompt: task_prompt.to_string(),
                steps: vec![TrajectoryStep {
                    turn: 1,
                    thought: Some("Using strict schema".to_string()),
                    tool_name: Some("extract".to_string()),
                    tool_input: Some("{\"user_id\": 42}".to_string()),
                    tool_output: Some("OK".to_string()),
                    error: None,
                }],
                final_output: Some("{\"user_id\": 42}".to_string()),
                succeeded: true,
                score: 0.95,
            })
        } else {
            // Reference run and Domain candidate both fail (0.20)
            Ok(Trajectory {
                task_prompt: task_prompt.to_string(),
                steps: vec![TrajectoryStep {
                    turn: 1,
                    thought: Some("Guessing args".to_string()),
                    tool_name: Some("extract".to_string()),
                    tool_input: Some("{}".to_string()),
                    tool_output: None,
                    error: Some("missing user_id".to_string()),
                }],
                final_output: None,
                succeeded: false,
                score: 0.20,
            })
        }
    }

    async fn propose_candidates(
        &self,
        _workspace: &HarnessWorkspace,
        _compressed_ref: &CompressedTrajectory,
        _experience_log: &str,
    ) -> Result<(HarnessPatch, HarnessPatch), HarnessError> {
        let invariant = HarnessPatch {
            kind: CandidateKind::Invariant,
            diffs: vec![ComponentDiff {
                component: HarnessComponent::ToolGuidance,
                updated_content: "PATCHED: Always enforce STRICT_USER_ID_SCHEMA.".to_string(),
                rationale: "Fix missing user_id in tool call".to_string(),
            }],
        };
        let domain = HarnessPatch {
            kind: CandidateKind::Domain,
            diffs: vec![ComponentDiff {
                component: HarnessComponent::Memory,
                updated_content: "PATCHED: Irrelevant domain note.".to_string(),
                rationale: "Add note".to_string(),
            }],
        };
        Ok((invariant, domain))
    }

    async fn compose(
        &self,
        _workspace: &HarnessWorkspace,
        _ref_trace: &CompressedTrajectory,
        invariant_patch: &HarnessPatch,
        invariant_trace: &CompressedTrajectory,
        _domain_patch: &HarnessPatch,
        _domain_trace: &CompressedTrajectory,
    ) -> Result<CompositionDecision, HarnessError> {
        let succeeded = invariant_trace.summary.contains("0.95");
        Ok(CompositionDecision {
            adopted_diffs: if succeeded {
                invariant_patch.diffs.clone()
            } else {
                vec![]
            },
            lesson_learned: "Enforce STRICT_USER_ID_SCHEMA in ToolGuidance for extraction tasks."
                .to_string(),
            winning_trajectory: Trajectory {
                task_prompt: "Extract user_id".to_string(),
                steps: vec![],
                final_output: Some("{\"user_id\": 42}".to_string()),
                succeeded,
                score: if succeeded { 0.95 } else { 0.20 },
            },
        })
    }
}

// ============================================================================
// TEST 5: Propose-Probe-Compose Adopts Winning Candidate & Logs Experience
// ============================================================================
#[tokio::test]
async fn test_ppc_loop_adopts_winning_candidate_and_updates_experience() {
    let base_dir = TempDir::new().expect("base dir");
    let probe_dir = TempDir::new().expect("probe dir");
    let base_ws = HarnessWorkspace::init(base_dir.path()).expect("init base");

    let original_memory = base_ws.read_component(HarnessComponent::Memory).unwrap();
    let engine = HarnessEngine::new(ScriptedMockDriver { both_regress: false }, 1000);

    let decision = engine
        .run_propose_probe_compose(&base_ws, probe_dir.path(), "Extract user_id")
        .await
        .expect("PPC execution");

    assert!(decision.winning_trajectory.succeeded);
    assert_eq!(decision.adopted_diffs.len(), 1);
    assert_eq!(
        base_ws
            .read_component(HarnessComponent::ToolGuidance)
            .unwrap(),
        "PATCHED: Always enforce STRICT_USER_ID_SCHEMA."
    );
    assert_eq!(
        base_ws.read_component(HarnessComponent::Memory).unwrap(),
        original_memory,
        "Losing Domain candidate diff must not be applied to base workspace"
    );

    let experience = base_ws.read_experience().expect("read experience");
    assert!(
        experience.contains("Enforce STRICT_USER_ID_SCHEMA"),
        "Winning lesson must be persisted to EXPERIENCE.md"
    );
}

// ============================================================================
// TEST 6: Propose-Probe-Compose Rejects Regressions
// ============================================================================
#[tokio::test]
async fn test_ppc_loop_discards_all_patches_when_both_candidates_regress() {
    let base_dir = TempDir::new().expect("base dir");
    let probe_dir = TempDir::new().expect("probe dir");
    let base_ws = HarnessWorkspace::init(base_dir.path()).expect("init base");
    let initial_snapshot = base_ws.snapshot().expect("initial snapshot");

    let engine = HarnessEngine::new(ScriptedMockDriver { both_regress: true }, 1000);

    let decision = engine
        .run_propose_probe_compose(&base_ws, probe_dir.path(), "Extract user_id")
        .await
        .expect("PPC execution");

    assert!(
        decision.adopted_diffs.is_empty(),
        "No diffs should be adopted when candidate scores regress below reference score"
    );
    let final_snapshot = base_ws.snapshot().expect("final snapshot");
    assert_eq!(
        initial_snapshot, final_snapshot,
        "Base workspace must remain 100% untouched when candidates regress"
    );
}

// ============================================================================
// PHASE 2 TDD SUITE: Ollama Driver (LFM2.5-350M) & FileMounter
// ============================================================================
use crate::{
    FileMounter, MountOutcome, MountTarget, OllamaConfig, OllamaDriver,
};
use std::fs;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// TEST 7: Ollama /api/tags Model Discovery
// ============================================================================
#[tokio::test]
async fn test_ollama_list_models_parses_api_tags() {
    let server = MockServer::start().await;

    let tags_payload = serde_json::json!({
        "models": [
            {
                "name": "lfm2.5-350m:latest",
                "size": 367001600u64,
                "digest": "sha256:lfm350m",
                "modified_at": "2026-10-22T00:00:00Z"
            },
            {
                "name": "qwen2.5-coder:1.5b",
                "size": 986000000u64,
                "digest": "sha256:qwen15b",
                "modified_at": "2026-10-21T00:00:00Z"
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tags_payload))
        .mount(&server)
        .await;

    let config = OllamaConfig {
        base_url: server.uri(),
        ..Default::default()
    };
    let driver = OllamaDriver::new(config);

    let models = driver.list_models().await.expect("list_models");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].name, "lfm2.5-350m:latest");
    assert_eq!(models[0].size, 367001600);
}

// ============================================================================
// TEST 8: Ollama /api/chat Task Execution with Mounted 8-File Context
// ============================================================================
#[tokio::test]
async fn test_ollama_execute_task_sends_mounted_harness_and_parses_json_response() {
    let server = MockServer::start().await;
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    ws.write_component(
        HarnessComponent::ToolGuidance,
        "UNIQUE_TOOL_RULE_42: Always pass user_id as integer.",
    )
    .expect("write tool guidance");

    let lfm_json_reply = serde_json::json!({
        "thought": "Applying UNIQUE_TOOL_RULE_42",
        "tool_name": "lookup_user",
        "tool_input": "{\"user_id\": 42}",
        "tool_output": "{\"status\": \"active\"}",
        "error": null,
        "final_output": "{\"user_id\": 42, \"status\": \"active\"}",
        "succeeded": true,
        "score": 0.95
    });

    let ollama_chat_envelope = serde_json::json!({
        "model": "lfm2.5-350m",
        "message": {
            "role": "assistant",
            "content": lfm_json_reply.to_string()
        },
        "done": true
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_string_contains("lfm2.5-350m"))
        .and(body_string_contains("UNIQUE_TOOL_RULE_42"))
        .and(body_string_contains("\"format\":\"json\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(ollama_chat_envelope))
        .mount(&server)
        .await;

    let driver = OllamaDriver::new(OllamaConfig {
        base_url: server.uri(),
        ..Default::default()
    });

    let trajectory = driver
        .execute_task(&ws, "Fetch status for user 42")
        .await
        .expect("execute_task");

    assert!(trajectory.succeeded);
    assert!((trajectory.score - 0.95).abs() < f32::EPSILON);
    assert_eq!(trajectory.steps.len(), 1);
    assert_eq!(
        trajectory.steps[0].tool_name.as_deref(),
        Some("lookup_user")
    );
}

// ============================================================================
// TEST 9: Markdown-Fenced JSON Sanitization & Malformed Output Handling
// ============================================================================
#[tokio::test]
async fn test_ollama_propose_and_compose_recover_from_markdown_fenced_or_malformed_json() {
    let server = MockServer::start().await;
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    // 350M models frequently wrap structured outputs in ```json ... ``` fences
    let fenced_propose_content = r#"```json
{
  "invariant_diffs": [
    {
      "component": "ToolGuidance",
      "updated_content": "Always validate JSON keys.",
      "rationale": "Prevent missing key error"
    }
  ],
  "domain_diffs": [
    {
      "component": "Memory",
      "updated_content": "User IDs are positive integers.",
      "rationale": "Domain schema rule"
    }
  ]
}
```"#;

    let chat_envelope = serde_json::json!({
        "model": "lfm2.5-350m",
        "message": {
            "role": "assistant",
            "content": fenced_propose_content
        },
        "done": true
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_envelope))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let driver = OllamaDriver::new(OllamaConfig {
        base_url: server.uri(),
        ..Default::default()
    });

    let dummy_compressed = CompressedTrajectory {
        summary: "TASK: test\nOUTCOME: succeeded=false score=0.10 final=None\n".to_string(),
        char_len: 56,
        estimated_tokens: 14,
    };

    let (inv_patch, dom_patch) = driver
        .propose_candidates(&ws, &dummy_compressed, "")
        .await
        .expect("propose_candidates should strip markdown fences");

    assert_eq!(inv_patch.diffs.len(), 1);
    assert_eq!(inv_patch.diffs[0].component, HarnessComponent::ToolGuidance);
    assert_eq!(dom_patch.diffs.len(), 1);
    assert_eq!(dom_patch.diffs[0].component, HarnessComponent::Memory);

    // Next, verify completely broken non-JSON returns a clean DriverError instead of panicking
    let broken_envelope = serde_json::json!({
        "model": "lfm2.5-350m",
        "message": {
            "role": "assistant",
            "content": "Sorry, I cannot output JSON right now."
        },
        "done": true
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(broken_envelope))
        .mount(&server)
        .await;

    let err_res = driver.propose_candidates(&ws, &dummy_compressed, "").await;
    assert!(
        matches!(err_res, Err(HarnessError::DriverError(_))),
        "Expected DriverError on unparseable model output, got: {:?}",
        err_res
    );
}

// ============================================================================
// TEST 10: FileMounter Routes to Prompt Context & Injects into Chat Request
// ============================================================================
#[tokio::test]
async fn test_file_mounter_routes_to_prompt_context_and_injects_into_chat_payload() {
    let server = MockServer::start().await;
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    let external_file = temp.path().join("invoice_data.json");
    fs::write(&external_file, "{\"invoice_id\": \"INV-9000\"}").expect("write external file");

    let driver = OllamaDriver::new(OllamaConfig {
        base_url: server.uri(),
        ..Default::default()
    });

    let outcome = FileMounter::mount_file(
        &ws,
        &driver,
        &external_file,
        MountTarget::PromptContext,
    )
    .expect("mount file to prompt context");

    assert!(matches!(outcome, MountOutcome::StagedInContext(_)));
    assert_eq!(driver.staged_context_files().len(), 1);
    assert_eq!(driver.staged_context_files()[0].filename, "invoice_data.json");

    let reply = serde_json::json!({
        "model": "lfm2.5-350m",
        "message": {
            "role": "assistant",
            "content": serde_json::json!({
                "thought": "Read invoice_data.json",
                "tool_name": null,
                "tool_input": null,
                "tool_output": null,
                "error": null,
                "final_output": "INV-9000",
                "succeeded": true,
                "score": 1.0
            }).to_string()
        },
        "done": true
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_string_contains("invoice_data.json"))
        .and(body_string_contains("INV-9000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(reply))
        .mount(&server)
        .await;

    let traj = driver
        .execute_task(&ws, "Extract invoice_id from attached file")
        .await
        .expect("execute_task with attached file");

    assert_eq!(traj.final_output.as_deref(), Some("INV-9000"));
}

// ============================================================================
// TEST 11: FileMounter Routes to 8-File Harness Slot & Validates Subagents JSON
// ============================================================================
#[test]
fn test_file_mounter_routes_to_harness_slot_and_validates_subagents() {
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");
    let driver = OllamaDriver::new(OllamaConfig::default());

    let custom_guardrails = temp.path().join("strict_guardrails.md");
    fs::write(&custom_guardrails, "# Custom Guardrails\nStrict mode enabled.")
        .expect("write guardrails file");

    let outcome = FileMounter::mount_file(
        &ws,
        &driver,
        &custom_guardrails,
        MountTarget::HarnessSlot(HarnessComponent::Guardrails),
    )
    .expect("mount to Guardrails slot");

    assert_eq!(
        outcome,
        MountOutcome::MountedToComponent(HarnessComponent::Guardrails)
    );
    assert_eq!(
        ws.read_component(HarnessComponent::Guardrails).unwrap(),
        "# Custom Guardrails\nStrict mode enabled."
    );

    // Mounting invalid JSON into the Subagents slot must fail and preserve existing subagents.json
    let broken_subagents = temp.path().join("broken_subagents.json");
    fs::write(&broken_subagents, "{ not_valid_json ").expect("write broken json");

    let err = FileMounter::mount_file(
        &ws,
        &driver,
        &broken_subagents,
        MountTarget::HarnessSlot(HarnessComponent::Subagents),
    );
    assert!(matches!(
        err,
        Err(HarnessError::InvalidComponentFormat {
            component: HarnessComponent::Subagents,
            ..
        })
    ));
}

// ============================================================================
// TEST 12: Live Local Ollama LFM2.5-350M Smoke Gate (Run with --ignored)
// ============================================================================
#[tokio::test]
#[ignore = "Requires local Ollama server running LiquidAI/lfm2.5-350m"]
async fn test_live_ollama_lfm2_5_350m_smoke_gate() {
    let base_dir = TempDir::new().expect("base dir");
    let probe_dir = TempDir::new().expect("probe dir");
    let ws = HarnessWorkspace::init(base_dir.path()).expect("init ws");

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "lfm2.5-350m".to_string());
    let driver = OllamaDriver::new(OllamaConfig {
        executor_model: model.clone(),
        editor_model: model,
        ..Default::default()
    });

    let engine = HarnessEngine::new(driver, 900);
    let decision = engine
        .run_propose_probe_compose(
            &ws,
            probe_dir.path(),
            "Return JSON with user_id=7 from text: 'User 7 logged in'",
        )
        .await
        .expect("live LFM2.5-350M PPC run");

    assert!(!decision.winning_trajectory.task_prompt.is_empty());
}

// ============================================================================
// TEST 13: End-to-End Currency Tool Guidance Self-Audit Plan Mode Benchmark
// ============================================================================
#[tokio::test]
async fn test_currency_invoice_self_audit_plan_mode_e2e() {
    let server = MockServer::start().await;
    let base_dir = TempDir::new().expect("base dir");
    let probe_dir = TempDir::new().expect("probe dir");
    let ws = HarnessWorkspace::init(base_dir.path()).expect("init ws");

    let flawed_guidance = "# Tool Guidance (Legacy v1 — Incomplete)\n\
When charging a customer invoice, invoke charge_invoice with amount as string.\n\
If missing integer amount_cents, set error=GATEWAY_REJECTED and score=0.25.";

    ws.write_component(HarnessComponent::ToolGuidance, flawed_guidance)
        .expect("write flawed guidance");

    let driver = OllamaDriver::new(OllamaConfig {
        base_url: server.uri(),
        ..Default::default()
    });

    // 1. Reference Run & Domain Probe (Legacy v1 Guidance): score 0.25
    let ref_reply = serde_json::json!({
        "thought": "tool_guidance lacks amount_cents",
        "tool_name": "charge_invoice",
        "tool_input": "{\"customer_id\": \"CUST-8842\", \"amount\": \"$125.50\"}",
        "tool_output": null,
        "error": "GATEWAY_REJECTED: tool_guidance.md is missing integer amount_cents and uppercase currency rules",
        "final_output": null,
        "succeeded": false,
        "score": 0.25
    });

    // 2. Propose: Invariant diff updates ToolGuidance to v2 rules
    let propose_reply = serde_json::json!({
        "invariant_diffs": [
            {
                "component": "ToolGuidance",
                "updated_content": "# Tool Guidance (v2 Upgraded)\ncharge_invoice requires integer amount_cents (12550) and uppercase currency (USD).",
                "rationale": "Fix gateway rejection"
            }
        ],
        "domain_diffs": [
            {
                "component": "Memory",
                "updated_content": "CUST-8842 uses USD.",
                "rationale": "Domain record"
            }
        ]
    });

    // 3. Invariant Probe (v2 Upgraded Guidance): score 0.98
    let inv_probe_reply = serde_json::json!({
        "thought": "ToolGuidance now specifies amount_cents=12550 and currency=USD",
        "tool_name": "charge_invoice",
        "tool_input": "{\"customer_id\": \"CUST-8842\", \"amount_cents\": 12550, \"currency\": \"USD\"}",
        "tool_output": "{\"status\": \"charged\"}",
        "error": null,
        "final_output": "{\"transaction_id\": \"TXN-99\", \"amount_cents\": 12550}",
        "succeeded": true,
        "score": 0.98
    });

    // 4. Compose: adopts ToolGuidance diff with winning_score 0.98
    let compose_reply = serde_json::json!({
        "adopted_diffs": [
            {
                "component": "ToolGuidance",
                "updated_content": "# Tool Guidance (v2 Upgraded)\ncharge_invoice requires integer amount_cents (12550) and uppercase currency (USD).",
                "rationale": "Fix gateway rejection"
            }
        ],
        "lesson_learned": "Always enforce integer amount_cents and uppercase currency in ToolGuidance.",
        "winning_score": 0.98,
        "final_output": "{\"transaction_id\": \"TXN-99\", \"amount_cents\": 12550}",
        "succeeded": true
    });

    // Mount Wiremock Handlers
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_string_contains("REFERENCE TRACE:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "lfm2.5-350m",
            "message": { "role": "assistant", "content": propose_reply.to_string() }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_string_contains("REF TRACE:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "lfm2.5-350m",
            "message": { "role": "assistant", "content": compose_reply.to_string() }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_string_contains("v2 Upgraded"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "lfm2.5-350m",
            "message": { "role": "assistant", "content": inv_probe_reply.to_string() }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_string_contains("Legacy v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model": "lfm2.5-350m",
            "message": { "role": "assistant", "content": ref_reply.to_string() }
        })))
        .mount(&server)
        .await;

    let engine = HarnessEngine::new(driver, 1200);
    let decision = engine
        .run_propose_probe_compose(
            &ws,
            probe_dir.path(),
            "Process invoice_8842.json with charge_invoice",
        )
        .await
        .expect("run PPC benchmark");

    // Assert that the self-healing occurred
    assert_eq!(decision.adopted_diffs.len(), 1);
    assert_eq!(decision.adopted_diffs[0].component, HarnessComponent::ToolGuidance);
    assert_eq!(decision.winning_trajectory.score, 0.98);

    // Assert that base workspace on disk is now permanently upgraded
    let disk_guidance = ws
        .read_component(HarnessComponent::ToolGuidance)
        .expect("read upgraded tool guidance");
    assert!(disk_guidance.contains("v2 Upgraded"));
    assert!(disk_guidance.contains("amount_cents (12550)"));

    // Assert that EXPERIENCE.md recorded the lesson learned
    let experience = ws.read_experience().expect("read experience");
    assert!(experience.contains("Always enforce integer amount_cents"));
}

// ============================================================================
// PHASE 4A TDD SUITE: GuardrailEnforcer, ScriptRunner, TaskEvaluator & ReAct
// ============================================================================
use crate::{
    GuardrailEnforcer, JsonFieldType, JsonSchemaEvaluator, ScriptRunner, TaskEvaluator,
};
use std::sync::Arc;

// ============================================================================
// TEST 14: GuardrailEnforcer Blocks Denied Tools, Patterns & Path Traversal
// ============================================================================
#[test]
fn test_guardrail_enforcer_blocks_forbidden_tools_and_path_traversal() {
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    let custom_guardrails = "\
# Guardrails
DENY_TOOL: drop_database
DENY_TOOL: rm_rf
DENY_ARG_PATTERN: --force-delete
";
    ws.write_component(HarnessComponent::Guardrails, custom_guardrails)
        .expect("write guardrails");

    // 1. Explicitly denied tool name must be blocked
    let denied_tool_res = GuardrailEnforcer::validate_tool_call(&ws, "rm_rf", "{\"target\": \"/tmp\"}");
    assert!(
        matches!(
            denied_tool_res,
            Err(HarnessError::GuardrailBlocked { ref tool_name, .. }) if tool_name == "rm_rf"
        ),
        "Expected GuardrailBlocked for rm_rf, got: {:?}",
        denied_tool_res
    );

    // 2. Denied argument pattern must be blocked
    let denied_arg_res = GuardrailEnforcer::validate_tool_call(
        &ws,
        "cleanup_logs",
        "{\"flag\": \"--force-delete\"}",
    );
    assert!(
        matches!(denied_arg_res, Err(HarnessError::GuardrailBlocked { .. })),
        "Expected GuardrailBlocked for --force-delete pattern, got: {:?}",
        denied_arg_res
    );

    // 3. Relative path traversal (../) must always be blocked by default
    let traversal_res = GuardrailEnforcer::validate_tool_call(
        &ws,
        "read_file",
        "{\"path\": \"../../etc/passwd\"}",
    );
    assert!(
        matches!(traversal_res, Err(HarnessError::GuardrailBlocked { .. })),
        "Expected GuardrailBlocked for path traversal, got: {:?}",
        traversal_res
    );

    // 4. Compliant tool call must pass
    let safe_res = GuardrailEnforcer::validate_tool_call(
        &ws,
        "charge_invoice",
        "{\"customer_id\": \"CUST-8842\", \"amount_cents\": 12550}",
    );
    assert!(safe_res.is_ok(), "Safe tool call should pass guardrails");
}

// ============================================================================
// TEST 15: ScriptRunner Parses Manifest & Executes Local Command with Input Env
// ============================================================================
#[tokio::test]
async fn test_script_runner_parses_manifest_and_executes_local_command() {
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    #[cfg(unix)]
    let (prog, args) = (
        "sh",
        vec![
            "-c",
            "printf '{\"echoed_input\":%s}' \"$HARNESS_TOOL_INPUT\"",
        ],
    );
    #[cfg(windows)]
    let (prog, args) = (
        "cmd",
        vec!["/C", "echo {\"echoed_input\":%HARNESS_TOOL_INPUT%}"],
    );

    let scripts_md = format!(
        "# Scripts Manifest\n```json\n{}\n```\n",
        serde_json::json!({
            "tools": [
                {
                    "name": "verify_invoice",
                    "description": "Echoes HARNESS_TOOL_INPUT back as JSON",
                    "program": prog,
                    "args": args
                }
            ]
        })
    );

    ws.write_component(HarnessComponent::Scripts, &scripts_md)
        .expect("write scripts manifest");

    let output = ScriptRunner::execute_tool(&ws, "verify_invoice", "{\"amount_cents\":12550}")
        .await
        .expect("execute verify_invoice tool");

    assert_eq!(output.exit_code, 0);
    assert!(
        output.stdout.contains("12550"),
        "Expected stdout to contain tool input 12550, got: {}",
        output.stdout
    );
}

// ============================================================================
// TEST 16: Deterministic JsonSchemaEvaluator Overrides Hallucinated LLM Score
// ============================================================================
#[test]
fn test_deterministic_json_evaluator_overrides_hallucinated_llm_score() {
    let evaluator = JsonSchemaEvaluator::new()
        .require_field("customer_id", JsonFieldType::String)
        .require_field("amount_cents", JsonFieldType::Integer)
        .require_field("currency", JsonFieldType::String);

    // Case A: LFM2.5-350M claims score=0.99 and succeeded=true, but emitted string "amount"
    let mut bad_traj = Trajectory {
        task_prompt: "Charge invoice".to_string(),
        steps: vec![TrajectoryStep {
            turn: 1,
            thought: Some("Done".to_string()),
            tool_name: None,
            tool_input: None,
            tool_output: None,
            error: None,
        }],
        final_output: Some("{\"customer_id\": \"CUST-8842\", \"amount\": \"$125.50\"}".to_string()),
        succeeded: true,
        score: 0.99,
    };

    evaluator.apply_to_trajectory(&mut bad_traj);
    assert!(
        !bad_traj.succeeded,
        "Evaluator must mark trajectory as failed when required integer field is missing"
    );
    assert!(
        bad_traj.score < 0.5,
        "Evaluator must downgrade hallucinated score (got {})",
        bad_traj.score
    );
    let step_err = bad_traj.steps[0]
        .error
        .as_deref()
        .expect("Evaluator should record validation error in last step");
    assert!(
        step_err.contains("amount_cents") && step_err.contains("currency"),
        "Validation error should name missing fields, got: {}",
        step_err
    );

    // Case B: Valid final_output matching all required fields & types scores 1.0
    let mut good_traj = Trajectory {
        task_prompt: "Charge invoice".to_string(),
        steps: vec![],
        final_output: Some(
            "{\"customer_id\": \"CUST-8842\", \"amount_cents\": 12550, \"currency\": \"USD\"}"
                .to_string(),
        ),
        succeeded: false,
        score: 0.10,
    };

    evaluator.apply_to_trajectory(&mut good_traj);
    assert!(good_traj.succeeded);
    assert!((good_traj.score - 1.0).abs() < f32::EPSILON);
}

// ============================================================================
// TEST 17: Multi-Turn ReAct Loop Runs ScriptRunner & Feeds Output to Turn 2
// ============================================================================
#[tokio::test]
async fn test_ollama_multiturn_react_loop_executes_script_and_feeds_output_back() {
    let server = MockServer::start().await;
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    #[cfg(unix)]
    let (prog, args) = ("sh", vec!["-c", "printf '{\"converted_cents\":12550}'"]);
    #[cfg(windows)]
    let (prog, args) = ("cmd", vec!["/C", "echo {\"converted_cents\":12550}"]);

    let scripts_md = serde_json::json!({
        "tools": [
            {
                "name": "dollars_to_cents",
                "description": "Converts dollar display string to integer cents",
                "program": prog,
                "args": args
            }
        ]
    })
    .to_string();
    ws.write_component(HarnessComponent::Scripts, &scripts_md)
        .expect("write scripts");

    // Turn 2 mock (matches only when prior observations are fed back in turn 2)
    let turn2_reply = serde_json::json!({
        "model": "lfm2.5-350m",
        "message": {
            "role": "assistant",
            "content": serde_json::json!({
                "thought": "Received converted_cents=12550 from tool",
                "tool_name": null,
                "tool_input": null,
                "tool_output": null,
                "error": null,
                "final_output": "{\"customer_id\": \"CUST-8842\", \"amount_cents\": 12550, \"currency\": \"USD\"}",
                "succeeded": true,
                "score": 0.80
            }).to_string()
        },
        "done": true
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_string_contains("PRIOR TOOL EXECUTION OBSERVATIONS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(turn2_reply))
        .mount(&server)
        .await;

    // Turn 1 mock (initial call requesting the `dollars_to_cents` tool)
    let turn1_reply = serde_json::json!({
        "model": "lfm2.5-350m",
        "message": {
            "role": "assistant",
            "content": serde_json::json!({
                "thought": "Need to convert $125.50 to cents using dollars_to_cents",
                "tool_name": "dollars_to_cents",
                "tool_input": "{\"display\": \"$125.50\"}",
                "tool_output": null,
                "error": null,
                "final_output": null,
                "succeeded": false,
                "score": 0.0
            }).to_string()
        },
        "done": true
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(turn1_reply))
        .mount(&server)
        .await;

    let evaluator = Arc::new(
        JsonSchemaEvaluator::new()
            .require_field("customer_id", JsonFieldType::String)
            .require_field("amount_cents", JsonFieldType::Integer)
            .require_field("currency", JsonFieldType::String),
    );

    let driver = OllamaDriver::new(OllamaConfig {
        base_url: server.uri(),
        ..Default::default()
    })
    .with_max_turns(3)
    .with_script_execution(true)
    .with_evaluator(evaluator);

    let traj = driver
        .execute_task(&ws, "Convert $125.50 and charge CUST-8842")
        .await
        .expect("multi-turn execute_task");

    assert_eq!(traj.steps.len(), 2, "Expected 2 turns in trajectory");
    assert!(
        traj.steps[0]
            .tool_output
            .as_deref()
            .unwrap_or("")
            .contains("converted_cents"),
        "Turn 1 step must record real stdout from ScriptRunner"
    );
    assert!(traj.succeeded);
    assert!(
        (traj.score - 1.0).abs() < f32::EPSILON,
        "Attached JsonSchemaEvaluator should upgrade valid final output score to 1.0"
    );
}

// ============================================================================
// TEST 18: Multi-Turn Loop Intercepts Guardrail Violation & Blocks Subprocess
// ============================================================================
#[tokio::test]
async fn test_ollama_multiturn_loop_records_guardrail_violation_and_blocks_subprocess() {
    let server = MockServer::start().await;
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    ws.write_component(
        HarnessComponent::Guardrails,
        "# Guardrails\nDENY_TOOL: wipe_disk\n",
    )
    .expect("write guardrails");

    let turn1_reply = serde_json::json!({
        "model": "lfm2.5-350m",
        "message": {
            "role": "assistant",
            "content": serde_json::json!({
                "thought": "Attempting restricted tool",
                "tool_name": "wipe_disk",
                "tool_input": "{}",
                "tool_output": null,
                "error": null,
                "final_output": null,
                "succeeded": false,
                "score": 0.0
            }).to_string()
        },
        "done": true
    });

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(turn1_reply))
        .mount(&server)
        .await;

    let driver = OllamaDriver::new(OllamaConfig {
        base_url: server.uri(),
        ..Default::default()
    })
    .with_max_turns(1)
    .with_script_execution(true);

    let traj = driver
        .execute_task(&ws, "Run restricted tool")
        .await
        .expect("execute_task should capture guardrail violation in trajectory step");

    assert!(!traj.succeeded);
    assert_eq!(traj.steps.len(), 1);
    let recorded_err = traj.steps[0]
        .error
        .as_deref()
        .expect("Guardrail violation must be recorded in step.error");
    assert!(
        recorded_err.contains("wipe_disk"),
        "Expected step.error to mention blocked tool 'wipe_disk', got: {}",
        recorded_err
    );
}

// ============================================================================
// PHASE 4B TDD SUITE (crates/ecoharness): Top-K Experience, Snapshot VCS & Progress
// ============================================================================
use crate::{ExperienceEntry, PpcStageEvent};

// ============================================================================
// TEST 18: Top-K EXPERIENCE.md Retrieval Filters Out Irrelevant Lessons
// ============================================================================
#[test]
fn test_experience_top_k_retrieval_filters_irrelevant_lessons() {
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    let entries = vec![
        ExperienceEntry {
            task_summary: "SQLite database migration schema".to_string(),
            adopted_components: vec![HarnessComponent::Scripts],
            lesson: "Always run PRAGMA foreign_keys=ON before migration.".to_string(),
        },
        ExperienceEntry {
            task_summary: "Stripe invoice currency conversion to cents".to_string(),
            adopted_components: vec![HarnessComponent::ToolGuidance],
            lesson: "Pass integer amount_cents and uppercase ISO currency USD.".to_string(),
        },
        ExperienceEntry {
            task_summary: "Kubernetes pod YAML readiness probe".to_string(),
            adopted_components: vec![HarnessComponent::Guardrails],
            lesson: "Always set initialDelaySeconds on readinessProbe.".to_string(),
        },
    ];

    for entry in &entries {
        ws.append_experience(entry).expect("append experience");
    }

    let retrieved = ws
        .retrieve_relevant_experience("Convert invoice currency USD to integer amount_cents", 1)
        .expect("retrieve top 1 relevant experience");

    assert!(
        retrieved.contains("amount_cents"),
        "Top-1 retrieved lesson must include the currency/invoice lesson, got: {}",
        retrieved
    );
    assert!(
        !retrieved.contains("Kubernetes"),
        "Irrelevant Kubernetes lesson must be excluded by top_k=1 filter"
    );
}

// ============================================================================
// TEST 19: Versioned Harness Snapshot Save & Rollback
// ============================================================================
#[test]
fn test_workspace_versioned_snapshot_save_and_rollback() {
    let temp = TempDir::new().expect("tempdir");
    let ws = HarnessWorkspace::init(temp.path()).expect("init ws");

    ws.write_component(
        HarnessComponent::SystemPrompt,
        "# v1 Baseline System Prompt",
    )
    .expect("write v1 prompt");

    let snap_path = ws
        .save_versioned_snapshot("v1_baseline")
        .expect("save v1_baseline snapshot");
    assert!(snap_path.exists(), "Snapshot file must be written to disk");

    // Mutate workspace to v2
    ws.write_component(
        HarnessComponent::SystemPrompt,
        "# v2 Experimental Broken Prompt",
    )
    .expect("write v2 prompt");
    assert_eq!(
        ws.read_component(HarnessComponent::SystemPrompt).unwrap(),
        "# v2 Experimental Broken Prompt"
    );

    // Roll back to v1_baseline
    ws.rollback_to_snapshot("v1_baseline")
        .expect("rollback to v1_baseline");
    assert_eq!(
        ws.read_component(HarnessComponent::SystemPrompt).unwrap(),
        "# v1 Baseline System Prompt"
    );
}

// ============================================================================
// TEST 20: PPC Loop With Progress Emits Ordered Stage Events
// ============================================================================
#[tokio::test]
async fn test_ppc_loop_with_progress_emits_ordered_stage_events() {
    let base_dir = TempDir::new().expect("base dir");
    let probe_dir = TempDir::new().expect("probe dir");
    let base_ws = HarnessWorkspace::init(base_dir.path()).expect("init base");

    let engine = HarnessEngine::new(ScriptedMockDriver { both_regress: false }, 1000);
    let mut events: Vec<PpcStageEvent> = Vec::new();

    let decision = engine
        .run_propose_probe_compose_with_progress(
            &base_ws,
            probe_dir.path(),
            "Extract user_id",
            |evt| events.push(evt),
        )
        .await
        .expect("PPC with progress");

    assert!(decision.winning_trajectory.succeeded);
    assert_eq!(
        events.len(),
        5,
        "Expected 5 ordered PpcStageEvents, got: {:?}",
        events
    );
    assert!(matches!(events[0], PpcStageEvent::ReferenceStarted));
    assert!(matches!(
        events[1],
        PpcStageEvent::ReferenceCompleted {
            succeeded: false,
            ..
        }
    ));
    assert!(matches!(
        events[2],
        PpcStageEvent::CandidatesProposed {
            invariant_diff_count: 1,
            domain_diff_count: 1
        }
    ));
    assert!(matches!(events[3], PpcStageEvent::ProbesCompleted { .. }));
    assert!(matches!(
        events[4],
        PpcStageEvent::Composed {
            adopted_diff_count: 1,
            ..
        }
    ));
}



