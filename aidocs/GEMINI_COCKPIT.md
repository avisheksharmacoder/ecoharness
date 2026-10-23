# Gemini Agent Cockpit & Cognitive Acceleration Matrix

> **Target Agent:** Google DeepMind / Google Cloud AI Gemini Models (Gemini 1.5 Pro, Gemini 2.0 Flash/Pro, Gemini Ultra, Antigravity IDE)  
> **Purpose:** Hyper-optimized, token-dense cognitive map and operational playbook engineered specifically for Gemini's long-context attention mechanisms and reasoning strengths.

---

## <gemini_agent_directives>

### 1. Architectural Mental Model
You are operating on **`harness2-rust`**, a clean-room Rust implementation of Google Cloud AI Research's **Harness²** architecture.

```text
               ┌────────────────────────────────────────────────────────┐
               │                   HarnessEngine (PPC)                  │
               └──────────┬─────────────────────────────┬───────────────┘
                          │                             │
               [Reference Pass]                         │
                          │                             ▼
                          ▼                   [OllamaDriver (LFM2.5)]
                  Score: S_ref                          │
                          │             ┌───────────────┴───────────────┐
                          ▼             ▼                               ▼
                 [Propose] ──► Invariant Candidate               Domain Candidate
                                        │                               │
                                        ▼                               ▼
                               [Sandbox Probing]               [Sandbox Probing]
                                        │                               │
                                        └───────────────┬───────────────┘
                                                        │
                                                        ▼
                                                    [Compose]
                                         S_win > S_ref ? Commit : Discard
                                                        │
                                                        ▼
                                               [EXPERIENCE.md]
                                            (Top-K Keyword Ranking)
```

### 2. High-Density Symbol & Line Map
Gemini, use this index to target exact line ranges with `view_file` or `replace_file_content` without wasting search rounds:

| Module | Core Structs & Functions | Key Invariants / Notes |
|---|---|---|
| [`crates/ecoharness/src/workspace.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/workspace.rs) | `HarnessComponent` (8 variants)<br>`HarnessWorkspace::init`<br>`HarnessWorkspace::load`<br>`HarnessWorkspace::apply_patch`<br>`HarnessWorkspace::retrieve_relevant_experience`<br>`HarnessWorkspace::save_versioned_snapshot`<br>`HarnessWorkspace::rollback_to_snapshot` | • Atomic rollback on invalid `subagents.json`.<br>• `.history/<tag>.json` versioning.<br>• Top-$K$ stopword filtering & keyword intersection. |
| [`crates/ecoharness/src/engine.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/engine.rs) | `HarnessEngine`<br>`PpcStageEvent` (5 ordered variants)<br>`HarnessEngine::run_propose_probe_compose_with_progress`<br>`CompositionDecision`<br>`ComponentDiff` | • Probes isolated in `probe_dir/probes/{invariant,domain}`.<br>• Regression rejection (discards all diffs if both candidates degrade).<br>• Emits all 5 stage milestones. |
| [`crates/ecoharness/src/guardrails.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/guardrails.rs) | `GuardrailEnforcer`<br>`GuardrailPolicy`<br>`validate_tool_call` | • Evaluates `DENY_TOOL:` and `DENY_ARG_PATTERN:`.<br>• Blocks path-traversal (`../`, `..\`).<br>• Must execute *before* subprocess spawn. |
| [`crates/ecoharness/src/scripts.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/scripts.rs) | `ScriptRunner`<br>`ScriptToolManifest`<br>`ScriptToolSpec`<br>`execute_script` | • Parses markdown code blocks for JSON manifest.<br>• Injects tool input into env var `HARNESS_TOOL_INPUT`.<br>• Captures stdout/stderr with exit code handling. |
| [`crates/ecoharness/src/subagents.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs) | `SubagentDispatcher`<br>`SubagentConfig`<br>`SubagentsManifest` | • Parses `subagents.json`.<br>• Dispatches `subagent:<name>` and `call_subagent` calls.<br>• Delegates subtasks to specialized models. |
| [`crates/ecoharness/src/plugins.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/plugins.rs) | `PluginRunner`<br>`PluginType` | • Parses `plugins/manifest.md`.<br>• Removes trailing commas from LLM JSON.<br>• Pre-prompt token budget & post-response JSON auto-repair. |
| [`crates/ecoharness/src/evaluator.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/evaluator.rs) | `JsonSchemaEvaluator`<br>`JsonFieldType`<br>`TaskEvaluator`<br>`EvaluationVerdict` | • Objective deterministic schema grading.<br>• Prevents LLM from hallucinating its own task score. |
| [`crates/ecoharness/src/mounter.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/mounter.rs) | `FileMounter`<br>`MountTarget`<br>`MountOutcome`<br>`mount_file` | • `PromptContext`: Stages into dynamic prompt.<br>• `HarnessSlot`: Overwrites one of the 8 canonical files.<br>• Validates JSON if target is `Subagents`. |
| [`crates/ecoharness/src/ollama.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/ollama.rs) | `OllamaDriver`<br>`OllamaConfig`<br>`ModelRole`<br>`execute_task`<br>`propose_candidates`<br>`compose_candidates` | • Targets `LiquidAI/lfm2.5-350m`.<br>• Strips markdown code-fences (````json ... ````).<br>• Recovers gracefully from malformed LLM responses. |
| [`crates/ecoharness-gui/src/state.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness-gui/src/state.rs) | `AppState`<br>`Message`<br>`ChatSession`<br>`ChatMessage`<br>`DetailPaneView`<br>`ExecutionStatus` | • Elm update loop.<br>• Multi-line `text_editor::Content`.<br>• Auto-persists and reloads conversations from `sessions.json`.<br>• Streams `PpcStageEvent` live. |
| [`crates/ecoharness-gui/src/view.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness-gui/src/view.rs) | `view`<br>`render_left_pane`<br>`render_detail_subpane`<br>`render_right_pane` | • Left pane: Sessions + Ollama Models + 8-File Inspector / Model Config + Experience preview + Temperature slider.<br>• Right pane: Top header + Plan Mode toggle + Chat stream + Add File + Composer. |

---

## <antigravity_gemini_tool_playbook>

### 1. Windows Shell & Path Conventions
* **OS:** Windows. Always use forward slashes in Markdown links (`file:///C:/RustProjects/harness2-rust/...`).
* **Powershell & Command execution:**
  * Avoid interactive commands.
  * Use `cargo test --workspace` (deterministic, 0 network).
  * Build checks: `cargo check --workspace` or `cargo check -p ecoharness -p ecoharness-gui`.

### 2. Precise Single-Step Editing
* For contiguous edits, prefer `replace_file_content`. Ensure `StartLine` and `EndLine` enclose the exact character sequence including whitespace.
* For non-contiguous edits, use `multi_replace_file_content`.

### 3. WireMock Recipe for Fast Test Additions
Gemini, when writing a test requiring LLM mock responses, use this exact pattern:
```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

let server = MockServer::start().await;
Mock::given(method("POST"))
    .and(path("/api/chat"))
    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "message": {
            "role": "assistant",
            "content": "{\"task_completed\": true, \"final_output\": \"...\", \"score\": 1.0}"
        }
    })))
    .mount(&server)
    .await;
```

---

## <gemini_specialized_patterns>

### The 8-File Invariant Enforcement
Whenever mutating the workspace, always honor:
1. `SystemPrompt` $\rightarrow$ `system_prompt.md`
2. `Guardrails` $\rightarrow$ `guardrails.md`
3. `Memory` $\rightarrow$ `memory.md`
4. `ToolGuidance` $\rightarrow$ `tool_guidance.md`
5. `Subagents` $\rightarrow$ `subagents.json`
6. `Skills` $\rightarrow$ `skills/manifest.md`
7. `Scripts` $\rightarrow$ `scripts/manifest.md`
8. `Plugins` $\rightarrow$ `plugins/manifest.md`

### Guardrail Safety Check Ordering
Always maintain this execution order:
$$\text{Incoming Tool Call} \xrightarrow{\quad} \text{GuardrailEnforcer} \xrightarrow{\quad} \begin{cases} \text{DENY} \implies \text{Trajectory Error Step} \\ \text{ALLOW} \implies \text{ScriptRunner Subprocess} \end{cases}$$

</gemini_agent_directives>
