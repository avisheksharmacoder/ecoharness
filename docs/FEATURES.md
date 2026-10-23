# EcoHarness / Harness² Architecture — System Feature Matrix

This document provides a comprehensive catalog of all features and architectural capabilities developed in the `harness2-rust` repository across both workspace crates: **`ecoharness`** (Core Headless SDK & Orchestration Engine) and **`ecoharness-gui`** (Native Desktop Studio).

---

## Workspace Structure

The project is organized as a unified Rust 2021 workspace with strict isolation between the headless core SDK and the GUI desktop application:

```text
harness2-rust/
├── Cargo.toml                  # Workspace manifest with shared dependencies
├── Cargo.lock
├── docs/
│   ├── README.md               # Documentation portal & index
│   ├── IMPLEMENTATION.md       # Deep-dive architecture & implementation guide
│   ├── FEATURES.md             # System feature catalog (this file)
│   └── PLANS.md                # System architectural roadmap & verification status
└── crates/
    ├── ecoharness/             # Core headless SDK, engine, drivers, mounter & plugins
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs          # Public API exports & HarnessError definitions
    │       ├── workspace.rs    # 8-file component workspace, Top-K retrieval & snapshot VCS
    │       ├── trajectory.rs   # Turn logging & token/char budget compressor (<1200 chars)
    │       ├── engine.rs       # Recursive Propose-Probe-Compose orchestrator & cleanup
    │       ├── guardrails.rs   # Security policy enforcer (deny tools, patterns, path traversal)
    │       ├── scripts.rs      # Subprocess command execution runner (scripts/manifest.md)
    │       ├── evaluator.rs    # Deterministic TaskEvaluator & JsonSchemaEvaluator
    │       ├── subagents.rs    # SubagentDispatcher parsing subagents.json & delegating tasks
    │       ├── plugins.rs      # PluginRunner (<think> stripping, comma repair, prompt budget)
    │       ├── mounter.rs      # Universal file routing (PromptContext vs HarnessSlot)
    │       ├── ollama.rs       # HTTP Ollama driver with LFM2.5 JSON contracts & multi-turn ReAct
    │       └── tests.rs        # SDK test suite (23 unit tests passed, 1 live hardware ignored)
    └── ecoharness-gui/         # Native 2-pane desktop studio (Iced 0.14 + Lucide)
        ├── Cargo.toml
        └── src/
            ├── main.rs         # Application entrypoint with startup restore & Lucide font
            ├── state.rs        # Elm AppState, Message enum, session restore & update logic
            ├── view.rs         # 2-pane responsive Iced UI implementation with editor & sliders
            └── tests.rs        # Headless GUI test suite (13 unit tests passed)
```

---

## 1. Core SDK: `ecoharness`

The `ecoharness` crate is a 100% headless, pure-Rust implementation of Google Cloud AI Research's **Harness²** recursive self-improving agent framework, optimized to run with deterministic reliability on local compact language models such as Liquid AI's `LFM2.5-350M`.

### 1.1. Eight-Component File-Mounted Workspace (`workspace.rs`)
* **Canonical 8-Component Architecture**:
  1. `SystemPrompt` (`system_prompt.md`): Baseline persona, system instructions, and operating mode.
  2. `Guardrails` (`guardrails.md`): Security boundaries, denied tool names, forbidden argument regexes.
  3. `Memory` (`memory.md`): Working domain memory, environment facts, persistent records.
  4. `ToolGuidance` (`tool_guidance.md`): Strict tool parameter specifications and failure-prevention rules.
  5. `Subagents` (`subagents.json`): Registry of specialized subagents, models, and isolated system prompts.
  6. `Skills` (`skills/manifest.md`): High-level cognitive procedures and problem-solving recipes.
  7. `Scripts` (`scripts/manifest.md`): Structured tool manifests mapping tool names to local executables.
  8. `Plugins` (`plugins/manifest.md`): Active middleware directives for prompt truncation and output repair.
* **Workspace Lifecycle**:
  * `HarnessWorkspace::init(root)`: Creates the directory structure, writes non-empty default content for all 8 components, and creates `EXPERIENCE.md`.
  * `HarnessWorkspace::load(root)`: Validates that all 8 components and `EXPERIENCE.md` exist on disk, parsing `subagents.json` to verify schema integrity.
* **Deterministic Sandbox Cloning & Isolation**:
  * `snapshot()`: Produces an in-memory `BTreeMap<HarnessComponent, String>` snapshot of all 8 files.
  * `clone_to_sandbox(sandbox_dir)`: Replicates the complete workspace into an isolated directory for candidate probing. Probe mutations never alter the base workspace.
* **Atomic Patch Application & Transactional Rollback**:
  * `apply_patch(&HarnessPatch)`: Applies multi-component diffs transactionally. If any file fails validation (e.g., broken JSON written to `Subagents`), all modified components automatically roll back to their pre-patch state.
* **Top-$K$ Keyword-Ranked Experience Retrieval**:
  * `retrieve_relevant_experience(query, top_k)`: Token-budget safe keyword retrieval across `EXPERIENCE.md`. Filters English stopwords, calculates keyword intersection scores against historical lessons, and returns only the top-$K$ most relevant entries to prevent context saturation in compact models.
* **Versioned Harness Snapshots & VCS Rollback**:
  * `save_versioned_snapshot(tag)`: Saves an immutable, timestamped checkpoint of all 8 components to `.history/<tag>.json`.
  * `rollback_to_snapshot(tag)`: Restores all 8 workspace files on disk directly from any named historical snapshot.

---

### 1.2. Trajectory Compression (`trajectory.rs`)
* **Execution Trace Modeling**:
  * `TrajectoryStep`: Models individual ReAct turns: `turn`, `thought`, `tool_name`, `tool_input`, `tool_output`, and `error`.
  * `Trajectory`: Captures overall task outcome: `task_prompt`, `steps`, `final_output`, `succeeded`, and `score`.
* **Budget-Constrained Compressor (`TrajectoryCompressor`)**:
  * Compresses raw multi-turn traces into a strict character limit (default `<= 1200` chars) to fit small model context windows.
  * **Strict Priority Guarantees**:
    1. Mandatory: Initial `task_prompt`.
    2. Critical: Steps containing `error.is_some()`.
    3. Mandatory: Final outcome (`succeeded`, `score`, `final_output`).
    4. Optional: Intermediate steps packed sequentially until the remaining character budget is exhausted.

---

### 1.3. Recursive Propose-Probe-Compose Engine (`engine.rs`)
* **Execution Trait Boundary (`ExecutionDriver`)**:
  * Decouples engine orchestration from LLM transport: `execute_task`, `propose_candidates`, and `compose`.
* **Ordered Live Stage Progress Events (`PpcStageEvent`)**:
  * `ReferenceStarted`: Reference trajectory begins on unmutated base harness.
  * `ReferenceCompleted { score, succeeded }`: Reference run concludes.
  * `CandidatesProposed { invariant_diff_count, domain_diff_count }`: Proposer generates orthogonal patches.
  * `ProbesCompleted { invariant_score, domain_score }`: Concurrent sandboxes finish evaluation.
  * `Composed { adopted_diff_count, winning_score }`: Composer synthesizes winning diffs.
* **Propose-Probe-Compose (PPC) Workflow (`HarnessEngine`)**:
  1. **Reference Run**: Executes task against base harness to obtain baseline trajectory.
  2. **Candidate Proposal**: Queries Editor model using compressed reference trace and Top-3 relevant experience lessons to propose orthogonal `Invariant` and `Domain` patches.
  3. **Concurrent Sandboxed Probing**: Clones two isolated sandboxes (`probe_invariant` and `probe_domain`), applies patches, and probes both concurrently via `tokio::try_join!`.
  4. **Contrastive Composition**: Analyzes reference and probe traces contrastively to compose winning diffs and formulate lessons learned.
  5. **Strict Regression Gate**: Diffs commit to base workspace and append to `EXPERIENCE.md` **only** if winning score strictly beats baseline score.
  6. **Automatic Rollback Checkpointing**: Saves an automatic `"pre_ppc_auto"` snapshot prior to committing diffs.
* **Ephemeral Probe Sandbox Cleanup**:
  * `with_cleanup_sandboxes(bool)`: Configures automatic cleanup.
  * `cleanup_probe_dirs(probe_root)`: Programmatically purges `probe_invariant` and `probe_domain` temporary directories upon PPC completion.

---

### 1.4. Multi-Turn ReAct Loop & Safety (`ollama.rs`, `guardrails.rs`, `scripts.rs`)
* **Guardrail Security Enforcer (`GuardrailEnforcer`)**:
  * Pre-execution security hook evaluated before any tool or subagent is dispatched.
  * Enforces `DENY_TOOL: <name>` and `DENY_ARG_PATTERN: <regex>`.
  * Blocks relative path traversal (`../`) to protect host filesystems.
* **Subprocess Script Execution (`ScriptRunner`)**:
  * Parses tool manifests declared in `scripts/manifest.md`.
  * Executes external programs via `tokio::process::Command` with environment variable injection (`HARNESS_TOOL_INPUT`).
  * Captures real `stdout`, `stderr`, and exit codes.
* **Multi-Turn ReAct Execution**:
  * `with_max_turns(usize)`: Limits maximum tool execution turns (default 1 to 3).
  * Executes tools sequentially, appending observations to the trajectory and feeding them back to the model in subsequent turns.
* **Objective Deterministic Scoring (`TaskEvaluator`)**:
  * `JsonSchemaEvaluator`: Enforces required JSON keys and types (`String`, `Integer`, `Float`, `Boolean`, `Array`, `Object`). Overrides hallucinated LLM scores when required fields or integer types are violated.

---

### 1.5. Subagent Delegation Dispatcher (`subagents.rs`)
* **Typed Delegation Registry**:
  * Parses `subagents.json` into typed [`SubagentSpec`](file:///c:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs#L5) definitions (`name`, `model`, `system_prompt`).
* **Tool Call Resolution**:
  * [`SubagentDispatcher::resolve_subagent`](file:///c:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs#L34): Intercepts `subagent:<name>` tool calls during ReAct turns.
* **Isolated Model & System Prompt Delegation**:
  * Dispatches queries directly to the subagent's designated Ollama model and private system prompt.
  * In the parent executor's system prompt, subagents are mounted only by tool name (`subagent:<name>`), preventing private system prompts from leaking and conserving token budget.

---

### 1.6. Middleware Plugin Engine (`plugins.rs`)
* **Directive Parsing**:
  * Parses `plugins/manifest.md` for active directives:
    * `ENABLE_PLUGIN: repair_trailing_commas`
    * `ENABLE_PLUGIN: strip_thinking_tags`
    * `MAX_PROMPT_CHARS: <usize>`
* **Pre-Prompt Token Budget Limiting**:
  * `apply_pre_prompt`: Truncates prompt text to `MAX_PROMPT_CHARS` to prevent context overflow on small models.
* **Post-Response Output Normalization**:
  * `apply_post_response`:
    * Strips `<think>...</think>` internal reasoning tags emitted by reasoning models.
    * Repairs trailing commas in JSON structures (`,}` or `,]`) outside string literals, ensuring seamless `serde_json` deserialization of compact model outputs.

---

### 1.7. Universal File Mounter (`mounter.rs`)
* **Dual Routing Mechanism (`FileMounter::mount_file`)**:
  * `MountTarget::PromptContext`: Reads external file into memory (`StagedContextFile`), injecting it into dynamic conversation payloads.
  * `MountTarget::HarnessSlot(component)`: Writes file directly into the designated 8-component harness file on disk, validating format rules (such as JSON syntax for `Subagents`).

---

## 2. Desktop GUI Studio: `ecoharness-gui`

The `ecoharness-gui` crate provides a native desktop studio built with `iced` 0.14, vector icons via `lucide-icons`, and native file dialogs via `rfd`.

### 2.1. Elm State Management & Persistence (`state.rs`)
* **State Machine (`AppState`)**:
  * Manages active `HarnessWorkspace`, probe path, and `OllamaDriver`.
  * Supports multiple `ChatSession`s and real-time execution tracking (`Idle`, `RunningSinglePass`, `RunningPlanModePpc`, `Error`).
  * Maintains multi-line editor content (`iced::widget::text_editor::Content`), snapshot lists, and live experience previews.
* **Automatic Startup Session Restoration**:
  * `AppState::new_with_startup_restore`: Auto-detects and loads `<workspace_root>/sessions.json` on launch, initializing the driver with multi-turn scripts, subagents, and plugins enabled.
* **On-Disk Persistence**:
  * `save_sessions()` / `load_sessions()`: Persists all conversation sessions, messages, scores, adopted diffs, and lessons learned to disk.
* **Diagnostic Inspector Telemetry**:
  * `inspector_summary()`: Formats real-time metrics (active temperature, script execution status, max turns, and top lesson line).
* **Snapshot Management Messages**:
  * `SaveSnapshotVersion(tag)`: Creates named snapshot checkpoints via GUI.
  * `RollbackSnapshotVersion(tag)`: Restores workspace to historical snapshots with immediate editor synchronization.

---

### 2.2. Two-Pane Desktop Interface (`view.rs`)

#### Left Pane: Master-Detail Sidebar
1. **Master List A (Chat Sessions)**:
   * Lists all sessions with active indicator markers (`● Session 1`).
   * `+ New` button creates fresh conversation threads.
2. **Master List B (Ollama Models)**:
   * Lists installed models discovered via `/api/tags` (`▸ lfm2.5-350m`).
   * `Sync` button re-queries local Ollama instance.
3. **Contextual Detail Sub-Pane**:
   * **8-File Harness Inspector**:
     * Grid of 8 component buttons (`SystemPrompt`, `Guardrails`, `Memory`, `ToolGuidance`, `Subagents`, `Skills`, `Scripts`, `Plugins`).
     * Real-time diagnostic summary header.
     * Save button, `Checkpoint v1`, and `Rollback v1` snapshot controls.
     * True multi-line text editor (`text_editor`) supporting selection, cursor movement, and editing.
   * **Ollama Model Config**:
     * Endpoint configuration (`http://localhost:11434`).
     * Role selectors (`Set Executor`, `Set Editor`, `Set Both`).
     * Temperature slider (`0.0`–`2.0`, step `0.05`).
     * Checkboxes for strict JSON schema and multi-turn scripts/subagents/plugins.

#### Right Pane: Chat, Plan Mode & Add File Workbench
1. **Header & Status Bar**:
   * Displays active Executor/Editor model pairing and live stage counter.
   * `Plan Mode (Recursive PPC)` toggler switch.
2. **Conversation Stream**:
   * Message cards with score indicators.
   * Plan Mode adopted diff badges displaying component names and rationales.
   * Highlighted `EXPERIENCE.md` lesson learned callouts.
3. **Composer & File Bar**:
   * Native file picker (`Add File`) via `rfd`.
   * Routing switches: `Route to Prompt` vs `Mount to [Component]`.
   * Staged file counter and `Clear` button.
   * Prompt input with keyboard submit (`Enter`) and `Send` button.

---

## 3. Test & Verification Matrix

The repository maintains **36 passing unit tests** (23 SDK + 13 GUI) and 1 ignored hardware smoke test:

| Crate | Test Name | Purpose | Result |
| :--- | :--- | :--- | :--- |
| `ecoharness` | `test_workspace_init_creates_all_8_components_and_experience_file` | Verifies disk creation of 8 components and `EXPERIENCE.md` | **Passed** |
| `ecoharness` | `test_workspace_sandbox_isolation_preserves_base_on_mutation` | Confirms sandbox mutations never alter the base workspace | **Passed** |
| `ecoharness` | `test_workspace_patch_rejects_invalid_subagents_json_and_rolls_back` | Validates transactional rollback upon invalid JSON in `subagents.json` | **Passed** |
| `ecoharness` | `test_trajectory_compressor_enforces_strict_budget_for_lfm_350m` | Verifies budget limits while preserving prompts and error lines | **Passed** |
| `ecoharness` | `test_ppc_loop_adopts_winning_candidate_and_updates_experience` | Verifies candidate adoption and `EXPERIENCE.md` persistence | **Passed** |
| `ecoharness` | `test_ppc_loop_discards_all_patches_when_both_candidates_regress` | Verifies strict regression gate discards failing candidate diffs | **Passed** |
| `ecoharness` | `test_ollama_list_models_parses_api_tags` | Validates wiremock parsing of Ollama `/api/tags` | **Passed** |
| `ecoharness` | `test_ollama_execute_task_sends_mounted_harness_and_parses_json_response` | Verifies harness mounting in `/api/chat` and JSON contract parsing | **Passed** |
| `ecoharness` | `test_ollama_propose_and_compose_recover_from_markdown_fenced_or_malformed_json` | Tests recovery from Markdown code fences and malformed outputs | **Passed** |
| `ecoharness` | `test_file_mounter_routes_to_prompt_context_and_injects_into_chat_payload` | Tests memory staging and prompt injection of user files | **Passed** |
| `ecoharness` | `test_file_mounter_routes_to_harness_slot_and_validates_subagents` | Tests direct slot writing and JSON validation | **Passed** |
| `ecoharness` | `test_currency_invoice_self_audit_plan_mode_e2e` | End-to-end self-healing currency schema benchmark (Plan Mode PPC) | **Passed** |
| `ecoharness` | `test_guardrail_enforcer_blocks_forbidden_tools_and_path_traversal` | Validates denial of forbidden tools, argument patterns, and path traversal | **Passed** |
| `ecoharness` | `test_script_runner_parses_manifest_and_executes_local_command` | Parses `scripts/manifest.md` and runs child subprocess with input env | **Passed** |
| `ecoharness` | `test_deterministic_json_evaluator_overrides_hallucinated_llm_score` | Evaluates required JSON fields/types, overriding hallucinated LLM score | **Passed** |
| `ecoharness` | `test_ollama_multiturn_react_loop_executes_script_and_feeds_output_back` | ReAct loop executes real script, feeds stdout back into turn 2 | **Passed** |
| `ecoharness` | `test_ollama_multiturn_loop_records_guardrail_violation_and_blocks_subprocess` | Pre-execution guardrail blocks forbidden tool and records error in step | **Passed** |
| `ecoharness` | `test_experience_top_k_retrieval_filters_irrelevant_lessons` | Keyword retrieval filters irrelevant lessons, preserving prompt budget | **Passed** |
| `ecoharness` | `test_workspace_versioned_snapshot_save_and_rollback` | Saves and restores named harness snapshot checkpoints | **Passed** |
| `ecoharness` | `test_ppc_loop_with_progress_emits_ordered_stage_events` | Verifies all 5 ordered `PpcStageEvent` milestones during PPC runs | **Passed** |
| `ecoharness` | `test_subagent_dispatcher_parses_manifest_and_executes_subagent_call` | Resolves subagent, delegates to target model, feeds output to turn 2 | **Passed** |
| `ecoharness` | `test_plugin_runner_repairs_trailing_commas_and_truncates_prompts` | Truncates long prompts, strips `<think>` tags, repairs trailing commas | **Passed** |
| `ecoharness` | `test_engine_with_cleanup_sandboxes_removes_probe_directories` | Verifies sandbox directories are removed from disk after compose | **Passed** |
| `ecoharness` | `test_live_ollama_lfm2_5_350m_smoke_gate` | Optional live hardware test against local Ollama daemon | *Ignored (Run with `--ignored`)* |
| `ecoharness-gui` | `test_left_pane_switching_between_chat_and_ollama_updates_detail_pane` | Tests sidebar list switching between session inspector and model config | **Passed** |
| `ecoharness-gui` | `test_left_detail_pane_harness_inspector_edits_and_saves_component_to_disk` | Tests GUI buffer editing and atomic saving to disk | **Passed** |
| `ecoharness-gui` | `test_right_pane_add_file_routes_to_prompt_context_or_harness_slot` | Tests GUI file attachment routing to prompt or slot | **Passed** |
| `ecoharness-gui` | `test_right_pane_normal_chat_mode_dispatches_single_pass_execution` | Tests single-pass turn dispatch and trajectory rendering | **Passed** |
| `ecoharness-gui` | `test_right_pane_plan_mode_runs_propose_probe_compose_and_records_diffs` | Tests recursive PPC execution, diff recording, and lesson capture | **Passed** |
| `ecoharness-gui` | `test_lucide_icon_mapping_for_navigation_and_modes` | Validates Lucide vector icon mappings | **Passed** |
| `ecoharness-gui` | `test_gui_multiline_text_editor_action_updates_and_saves_component` | Tests multi-line editor action processing and component saving | **Passed** |
| `ecoharness-gui` | `test_gui_ppc_stage_progress_events_update_status_and_history` | Tests UI stage history updates during live PPC events | **Passed** |
| `ecoharness-gui` | `test_gui_sessions_persist_to_and_reload_from_disk` | Verifies `sessions.json` serialization and deserialization | **Passed** |
| `ecoharness-gui` | `test_gui_snapshot_save_and_rollback_restores_inspector_buffer` | Verifies snapshot save and rollback through GUI messages | **Passed** |
| `ecoharness-gui` | `test_gui_startup_restore_loads_sessions_and_enables_multiturn_scripts` | Tests `new_with_startup_restore` loading sessions and driver flags | **Passed** |
| `ecoharness-gui` | `test_gui_toggle_script_execution_and_temperature_updates_driver` | Tests toggling scripts and temperature updating driver configuration | **Passed** |
| `ecoharness-gui` | `test_gui_live_ppc_batch_progress_populates_stage_history_and_experience` | Tests `ApplyLivePpcRun` populating stage history and lesson callouts | **Passed** |

---

## 4. Commands Quick Reference

### Run Full Test Suite Across All Crates
```bash
cargo test --workspace
```

### Run Core SDK Tests Only
```bash
cargo test -p ecoharness
```

### Run Live Ollama Hardware Smoke Test (Requires local Ollama running LFM2.5-350M)
```bash
cargo test -p ecoharness -- --ignored
```

### Run Headless GUI Tests Only
```bash
cargo test -p ecoharness-gui
```

### Check Full Workspace for Compile Correctness & Zero Warnings
```bash
cargo check --workspace --all-targets
```

### Launch the Native Desktop Studio
```bash
cargo run -p ecoharness-gui
```
