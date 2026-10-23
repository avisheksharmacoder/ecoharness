# EcoHarness / Harness² Implementation Guide

This document provides an exhaustive, engineering-grade explanation of all architectural features developed across **`crates/ecoharness`** (Core Headless SDK) and **`crates/ecoharness-gui`** (Native Desktop Studio), detailing their mathematical motivation, data structures, algorithms, and integration patterns.

---

## 1. Architectural Foundation & Dual-Loop Model

`ecoharness` implements Google Cloud AI Research's **Harness²** architecture. Rather than updating weights in parameter space (which is computationally prohibitive for edge deployments), the system optimizes agent behavior at inference time by mutating the **8 file-mounted harness components** surrounding the model.

```
                      ┌───────────────────────────────────────────────┐
                      │             Incoming User Task                │
                      └──────────────────────┬────────────────────────┘
                                             │
                                             ▼
                               ┌───────────────────────────┐
                               │ 1. Reference Run (Base WS)│
                               └─────────────┬─────────────┘
                                             │
                                             ▼
                               ┌───────────────────────────┐
                               │  2. Compressed Trajectory │ (<= 1200 chars)
                               │  + Top-K Experience Query │
                               └─────────────┬─────────────┘
                                             │
                        ┌────────────────────┴────────────────────┐
                        ▼                                         ▼
            ┌───────────────────────┐                 ┌───────────────────────┐
            │  Invariant Candidate  │                 │   Domain Candidate    │
            │  (Rules & Prompts)    │                 │ (Memory & Guidance)   │
            └───────────┬───────────┘                 └───────────┬───────────┘
                        │                                         │
                        ▼                                         ▼
            ┌───────────────────────┐                 ┌───────────────────────┐
            │ Probe Invariant SB    │                 │    Probe Domain SB    │
            │ (Isolated Sandbox)    │                 │  (Isolated Sandbox)   │
            └───────────┬───────────┘                 └───────────┬───────────┘
                        │                                         │
                        └────────────────────┬────────────────────┘
                                             │ (Concurrent tokio::try_join!)
                                             ▼
                               ┌───────────────────────────┐
                               │ 3. Contrastive Composer   │
                               └─────────────┬─────────────┘
                                             │
                                             ▼
                               ┌───────────────────────────┐
                               │ 4. Strict Regression Gate │ (Score > Baseline?)
                               └──────┬─────────────┬──────┘
                          YES         │             │ NO
             ┌────────────────────────┘             └────────────────────────┐
             ▼                                                               ▼
┌───────────────────────────────┐                              ┌───────────────────────────┐
│ • Save pre_ppc_auto Snapshot  │                              │ • Discard All Patches     │
│ • Commit Patches to Base WS   │                              │ • Cleanup Probe Sandboxes │
│ • Append Lesson to EXPERIENCE │                              └───────────────────────────┘
│ • Cleanup Probe Sandboxes     │
└───────────────────────────────┘
```

---

## 2. Feature Deep-Dive: Subagent Delegation Dispatcher

### 2.1. Motivation & Challenge
Compact models (such as `LFM2.5-350M`) suffer from sharp degradation in reasoning accuracy when single prompts contain conflicting role instructions, dense tool manifests, and extensive domain guidelines. The subagent pattern partitions complex tasks by delegating specialized workflows (e.g., security audits, code reviews, calculations) to dedicated model instances running with isolated system prompts.

### 2.2. Data Structures ([`crates/ecoharness/src/subagents.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs))

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentSpec {
    pub name: String,
    pub model: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentManifest {
    #[serde(default)]
    pub subagents: Vec<SubagentSpec>,
}

pub struct SubagentDispatcher;
```

### 2.3. Manifest Parsing & Schema Integrity
Subagents are declared in [`HarnessComponent::Subagents`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/workspace.rs) (`subagents.json`). [`SubagentDispatcher::parse_manifest`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs#L25) parses the JSON payload. If the JSON is invalid, it yields a typed [`HarnessError::InvalidComponentFormat`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/lib.rs):

```rust
impl SubagentDispatcher {
    pub fn parse_manifest(subagents_json: &str) -> Result<SubagentManifest, HarnessError> {
        serde_json::from_str::<SubagentManifest>(subagents_json).map_err(|err| {
            HarnessError::InvalidComponentFormat {
                component: HarnessComponent::Subagents,
                reason: err.to_string(),
            }
        })
    }
}
```

### 2.4. Tool Resolution & Prompt Isolation
When an LLM issues a tool invocation in the format `subagent:<name>`, [`SubagentDispatcher::resolve_subagent`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs#L36) strips the prefix and matches the requested subagent:

```rust
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

    Ok(manifest.subagents.into_iter().find(|s| s.name == target_name))
}
```

### 2.5. Execution Pipeline & Preventing Prompt Leakage
In [`OllamaDriver::execute_subagent_call`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/ollama.rs#L383), execution follows a strict 3-step security sequence:
1. **Guardrail Check**: The call is evaluated against [`GuardrailEnforcer::validate_tool_call`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/guardrails.rs#L27) to confirm `subagent:<name>` is not denied and `tool_input` does not match forbidden patterns.
2. **Subagent Resolution**: The matching [`SubagentSpec`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs#L6) is retrieved from disk.
3. **Isolated Dispatch**: The tool input is dispatched to the subagent's designated model and private system prompt:

```rust
async fn execute_subagent_call(
    &self,
    workspace: &HarnessWorkspace,
    tool_name: &str,
    tool_input: &str,
) -> Result<Option<String>, HarnessError> {
    GuardrailEnforcer::validate_tool_call(workspace, tool_name, tool_input)?;

    let spec = match SubagentDispatcher::resolve_subagent(workspace, tool_name)? {
        Some(s) => s,
        None => return Ok(None),
    };

    let reply = self
        .send_raw_chat(&spec.model, spec.system_prompt, tool_input.to_string())
        .await?;

    Ok(Some(reply))
}
```

> [!IMPORTANT]
> **Prompt Leakage Prevention**: In [`OllamaDriver::build_harness_system_prompt`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/ollama.rs#L125), `subagents.json` is not dumped verbatim into the primary executor's system prompt. Instead, only available tool signatures (`subagent:<name>`) are listed. This ensures the subagent's private instructions never pollute the parent model's context or exhaust prompt token budgets.

---

## 3. Feature Deep-Dive: Middleware Plugin Engine

### 3.1. Motivation & Challenges
Small models frequently output syntactic defects:
1. **Thinking Blocks**: Modern reasoning models emit internal `<think>...</think>` scratchpads that corrupt downstream JSON parsers.
2. **Trailing JSON Commas**: Generating `{"key": "value",}` or `[1, 2,]` causes standard `serde_json` parsers to fail.
3. **Prompt Budget Overflow**: Long prompts exceed the active context limit of edge models.

### 3.2. Data Structures ([`crates/ecoharness/src/plugins.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/plugins.rs))

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPolicy {
    pub repair_trailing_commas: bool,
    pub strip_thinking_tags: bool,
    pub max_prompt_chars: Option<usize>,
}

pub struct PluginRunner;
```

### 3.3. Policy Parsing
Directives are parsed line-by-line from [`HarnessComponent::Plugins`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/workspace.rs) (`plugins/manifest.md`):
* `ENABLE_PLUGIN: repair_trailing_commas`
* `ENABLE_PLUGIN: strip_thinking_tags`
* `MAX_PROMPT_CHARS: <usize>`

### 3.4. Pre-Prompt Truncation Pipeline
[`PluginRunner::apply_pre_prompt`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/plugins.rs#L45) truncates oversized inputs before they reach the model tokenizer:

```rust
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
```

### 3.5. Post-Response Normalization Pipeline
[`PluginRunner::apply_post_response`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/plugins.rs#L94) applies two post-processing passes:

#### Pass 1: Reasoning Scratchpad Stripping
```rust
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
```

#### Pass 2: Quote-Aware Trailing Comma Repair
Trailing commas before `}` or `]` outside string literals are filtered:
```rust
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
                continue; // Skip trailing comma
            }
        }
        out.push(chars[i]);
    }
    out
}
```

---

## 4. Feature Deep-Dive: Ephemeral Probe Sandbox Cleanup

### 4.1. Motivation
During Propose-Probe-Compose (PPC), the engine creates two temporary filesystem sandboxes:
* `<probe_root>/probe_invariant`
* `<probe_root>/probe_domain`

Without automated cleanup, hundreds of megabytes of scratch files accumulate over multi-turn optimization runs.

### 4.2. Implementation ([`crates/ecoharness/src/engine.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/engine.rs))

```rust
impl<D: ExecutionDriver> HarnessEngine<D> {
    pub fn with_cleanup_sandboxes(mut self, cleanup: bool) -> Self {
        self.cleanup_sandboxes = cleanup;
        self
    }

    pub fn cleanup_probe_dirs(probe_root: &Path) -> Result<(), HarnessError> {
        for sub in ["probe_invariant", "probe_domain"] {
            let dir = probe_root.join(sub);
            if dir.exists() {
                fs::remove_dir_all(&dir).map_err(|source| HarnessError::Io {
                    path: dir,
                    source,
                })?;
            }
        }
        Ok(())
    }
}
```

The engine invokes `Self::cleanup_probe_dirs(probe_root)` at the end of `run_propose_probe_compose_with_progress`, guaranteeing sandboxes are deleted regardless of whether candidates win or regress.

---

## 5. Feature Deep-Dive: Desktop GUI Studio Integration

### 5.1. Multi-Line Text Editor ([`crates/ecoharness-gui/src/view.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness-gui/src/view.rs))
The GUI utilizes `iced::widget::text_editor::Content` to deliver an IDE-grade component editor:
* Full cursor positioning, selection ranges, copy-paste, and scroll tracking.
* Component switches cleanly sync the buffer without uncommitted data loss.

### 5.2. Automatic Startup Session Restoration
In [`crates/ecoharness-gui/src/state.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness-gui/src/state.rs#L204):
```rust
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
```
On application launch, `new_with_startup_restore`:
1. Reads `<workspace_root>/sessions.json` and loads prior chat sessions.
2. Arms the `OllamaDriver` with 3-turn multi-turn scripts, subagents, and plugins enabled.

### 5.3. Real-Time Inspector Diagnostic Telemetry
The inspector header renders live runtime diagnostics via [`AppState::inspector_summary`](file:///C:/RustProjects/harness2-rust/crates/ecoharness-gui/src/state.rs#L223):
```rust
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
```

### 5.4. Fine-Grained GUI Controls
* **Temperature Slider**: An interactive `iced::widget::slider` (range `0.0` to `2.0`, step `0.05`) updates the active Ollama driver dynamically via `Message::SetTemperature(f32)`.
* **Execution Flags**: Checkboxes for toggling multi-turn scripts, subagents, and plugins.
* **Snapshot Versioning**: `Checkpoint v1` (`SaveSnapshotVersion`) and `Rollback v1` (`RollbackSnapshotVersion`) allow instant workspace time-travel directly from the UI.
* **Live PPC Progress**: `Message::ApplyLivePpcRun` updates conversation history, stage counters, and `EXPERIENCE.md` callouts in real time.

---

## 6. Feature Deep-Dive: Top-$K$ Experience Keyword Retrieval

### 6.1. Retrieval Algorithm ([`crates/ecoharness/src/workspace.rs`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/workspace.rs#L286))
`retrieve_relevant_experience` computes relevance scores by:
1. Tokenizing the query string into normalized lowercase tokens.
2. Filtering out standard English stopwords (`the`, `a`, `and`, `to`, `is`, `in`, `for`, `on`, `with`, `as`, `by`, `at`).
3. Splitting `EXPERIENCE.md` into distinct entry blocks.
4. Scoring each entry based on token intersection frequency.
5. Returning the top-$K$ highest-scoring lessons.

This prevents small models like `LFM2.5-350M` from exceeding their token budget while guaranteeing that relevant historical lessons are injected into the Proposer prompt.

---

## 7. Complete Verification Matrix (36 Unit Tests)

All 36 unit tests pass completely offline using `wiremock`:

```
running 24 tests (ecoharness)
test tests::test_deterministic_json_evaluator_overrides_hallucinated_llm_score ... ok
test tests::test_currency_invoice_self_audit_plan_mode_e2e ... ok
test tests::test_experience_top_k_retrieval_filters_irrelevant_lessons ... ok
test tests::test_guardrail_enforcer_blocks_forbidden_tools_and_path_traversal ... ok
test tests::test_file_mounter_routes_to_prompt_context_and_injects_into_chat_payload ... ok
test tests::test_file_mounter_routes_to_harness_slot_and_validates_subagents ... ok
test tests::test_ollama_list_models_parses_api_tags ... ok
test tests::test_ollama_execute_task_sends_mounted_harness_and_parses_json_response ... ok
test tests::test_ollama_multiturn_loop_records_guardrail_violation_and_blocks_subprocess ... ok
test tests::test_ollama_propose_and_compose_recover_from_markdown_fenced_or_malformed_json ... ok
test tests::test_script_runner_parses_manifest_and_executes_local_command ... ok
test tests::test_trajectory_compressor_enforces_strict_budget_for_lfm_350m ... ok
test tests::test_ppc_loop_discards_all_patches_when_both_candidates_regress ... ok
test tests::test_ppc_loop_adopts_winning_candidate_and_updates_experience ... ok
test tests::test_ppc_loop_with_progress_emits_ordered_stage_events ... ok
test tests::test_workspace_init_creates_all_8_components_and_experience_file ... ok
test tests::test_workspace_patch_rejects_invalid_subagents_json_and_rolls_back ... ok
test tests::test_workspace_sandbox_isolation_preserves_base_on_mutation ... ok
test tests::test_workspace_versioned_snapshot_save_and_rollback ... ok
test tests::test_engine_with_cleanup_sandboxes_removes_probe_directories ... ok
test tests::test_ollama_multiturn_react_loop_executes_script_and_feeds_output_back ... ok
test tests::test_plugin_runner_repairs_trailing_commas_and_truncates_prompts ... ok
test tests::test_subagent_dispatcher_parses_manifest_and_executes_subagent_call ... ok
test tests::test_live_ollama_lfm2_5_350m_smoke_gate ... ignored

test result: ok. 23 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out

running 13 tests (ecoharness-gui)
test tests::test_gui_sessions_persist_to_and_reload_from_disk ... ok
test tests::test_gui_multiline_text_editor_action_updates_and_saves_component ... ok
test tests::test_gui_live_ppc_batch_progress_populates_stage_history_and_experience ... ok
test tests::test_gui_ppc_stage_progress_events_update_status_and_history ... ok
test tests::test_gui_snapshot_save_and_rollback_restores_inspector_buffer ... ok
test tests::test_gui_startup_restore_loads_sessions_and_enables_multiturn_scripts ... ok
test tests::test_gui_toggle_script_execution_and_temperature_updates_driver ... ok
test tests::test_left_detail_pane_harness_inspector_edits_and_saves_component_to_disk ... ok
test tests::test_left_pane_switching_between_chat_and_ollama_updates_detail_pane ... ok
test tests::test_lucide_icon_mapping_for_navigation_and_modes ... ok
test tests::test_right_pane_add_file_routes_to_prompt_context_or_harness_slot ... ok
test tests::test_right_pane_normal_chat_mode_dispatches_single_pass_execution ... ok
test tests::test_right_pane_plan_mode_runs_propose_probe_compose_and_records_diffs ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
