# EcoHarness / Harness² Architecture — System Feature Matrix

This document provides a comprehensive catalog of all features and architectural capabilities developed in the `harness2-rust` repository across both crates: **`ecoharness`** (Core SDK & Engine) and **`ecoharness-gui`** (Native Desktop Studio).

---

## Workspace Structure

The project is organized as a unified Rust 2021 workspace:

```text
harness2-rust/
├── Cargo.toml                  # Workspace manifest with shared dependencies
├── Cargo.lock
├── docs/
│   └── FEATURES.md             # System feature documentation (this file)
└── crates/
    ├── ecoharness/             # Core Harness² SDK, engine, drivers & mounter
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs          # Public API exports & error definitions
    │       ├── workspace.rs    # 8-file component workspace & EXPERIENCE.md
    │       ├── trajectory.rs   # Turn logging & token/char budget compressor
    │       ├── engine.rs       # Recursive Propose-Probe-Compose orchestrator
    │       ├── mounter.rs      # Universal file routing (Prompt vs Slot)
    │       ├── ollama.rs       # HTTP Ollama driver with LFM2.5 JSON contracts
    │       └── tests.rs        # SDK test suite (11 passed, 1 live ignored)
    └── ecoharness-gui/         # Native 2-pane desktop studio (Iced 0.14)
        ├── Cargo.toml
        └── src/
            ├── main.rs         # Application entrypoint with Lucide font
            ├── state.rs        # Elm AppState, Message enum & update logic
            ├── view.rs         # 2-pane responsive Iced UI implementation
            └── tests.rs        # Headless GUI test suite (6 passed)
```

---

## 1. Core SDK: `ecoharness`

The `ecoharness` crate implements Google's **Harness²** recursive self-improving agent framework, optimized to run reliably on local compact language models such as LiquidAI's `LFM2.5-350M`.

### 1.1. Eight-Component File-Mounted Workspace (`workspace.rs`)
* **Standard 8-Component Architecture**:
  1. `SystemPrompt` (`system_prompt.md`): Baseline role instructions and operational persona.
  2. `Guardrails` (`guardrails.md`): Safety constraints, schema adherence rules, output validations.
  3. `Memory` (`memory.md`): Working memory, domain knowledge, task-specific facts.
  4. `ToolGuidance` (`tool_guidance.md`): Parameter specifications, error prevention rules for tools.
  5. `Subagents` (`subagents.json`): Delegation registry with strictly enforced JSON syntax.
  6. `Skills` (`skills/manifest.md`): Skill definitions, procedural workflows.
  7. `Scripts` (`scripts/manifest.md`): Mounted script references and usage guidelines.
  8. `Plugins` (`plugins/manifest.md`): Middleware hooks and plugin configurations.
* **Workspace Lifecycle**:
  * `HarnessWorkspace::init(root)`: Deterministically creates the directory structure, writes non-empty default content for all 8 components, and initializes `EXPERIENCE.md`.
  * `HarnessWorkspace::load(root)`: Validates that all 8 components and `EXPERIENCE.md` exist on disk, parsing `subagents.json` to guarantee schema integrity.
* **Deterministic Snapshotting & Sandbox Isolation**:
  * `snapshot()`: Captures an in-memory `BTreeMap<HarnessComponent, String>` snapshot of all components for rollback checkpoints and integrity diffs.
  * `clone_to_sandbox(sandbox_dir)`: Replicates the complete 8-component workspace and `EXPERIENCE.md` into an isolated probe directory. Mutations in sandboxes never taint the base workspace.
* **Atomic Patch Application & Rollback**:
  * `apply_patch(&HarnessPatch)`: Applies multi-component diffs transactionally. If any diff fails validation (e.g., malformed JSON in `subagents.json`), all modified components automatically roll back to their pre-patch state.
* **Lifelong Experience Logging**:
  * `append_experience(&ExperienceEntry)`: Appends structured lessons learned, task summaries, and adopted components to `EXPERIENCE.md`.

---

### 1.2. Trajectory Compression (`trajectory.rs`)
* **Execution Trace Modeling**:
  * `TrajectoryStep`: Models individual agent turns including `turn`, `thought`, `tool_name`, `tool_input`, `tool_output`, and `error`.
  * `Trajectory`: Captures overall task outcome, including `task_prompt`, step traces, `final_output`, `succeeded`, and `score`.
* **Budget-Constrained Compressor (`TrajectoryCompressor`)**:
  * Compresses raw execution trajectories into a strict character/token budget (e.g., `<= 1200` chars for 350M parameter models).
  * **Strict Priority Guarantees**:
    1. Mandatory: Always preserves the initial `task_prompt`.
    2. Critical: Always preserves steps where `error.is_some()`.
    3. Mandatory: Always preserves final outcome (`succeeded`, `score`, `final_output`).
    4. Optional: Packs normal intermediate steps into remaining budget until the limit is reached.
  * Estimates token consumption using ceiling char division (`char_len.div_ceil(4)`).

---

### 1.3. Recursive Propose-Probe-Compose Engine (`engine.rs`)
* **Execution Trait Boundary (`ExecutionDriver`)**:
  * Decouples the core engine logic from network/LLM dependencies: `execute_task`, `propose_candidates`, and `compose`.
* **Propose-Probe-Compose (PPC) Workflow (`HarnessEngine`)**:
  1. **Reference Run**: Runs the unmutated base harness to obtain baseline trajectory and score.
  2. **Candidate Proposal**: Analyzes compressed reference trajectory and `EXPERIENCE.md` to propose two orthogonal patches:
     * `Invariant`: General reasoning, planning, verification, and tool-handling improvements.
     * `Domain`: Task-specific schemas, domain facts, and environment constraints.
  3. **Concurrent Sandbox Probing**: Clones two isolated sandboxes (`probe_invariant` and `probe_domain`), applies each patch, and runs both probes concurrently via `tokio::try_join!`.
  4. **Contrastive Composition**: Feeds the reference trace, invariant probe trace, and domain probe trace into the model to synthesize winning diffs and extract lessons learned.
  5. **Strict Regression Gate**: Winning diffs are committed to the base workspace and logged to `EXPERIENCE.md` **only** if the winning score strictly improves upon the baseline reference score. If candidate scores regress, all patches are discarded and the base harness remains untouched.
* **Single-Pass Mode**:
  * `run_single_pass`: Bypasses the PPC loop for standard, direct conversational turns.

---

### 1.4. Ollama Driver & LFM2.5 Integration (`ollama.rs`)
* **Local Backend Integration**:
  * Asynchronous HTTP client via `reqwest` talking to local or remote Ollama servers (`http://localhost:11434`).
  * `list_models()`: Discovers locally installed models via `GET /api/tags`.
  * Configurable endpoint, temperature, and model parameters (`OllamaConfig`).
* **Multi-Role Model Assignment**:
  * `ModelRole::Executor`: Runs the task trajectory using the mounted harness.
  * `ModelRole::Editor`: Proposes candidate diffs and composes probe traces.
  * `ModelRole::Both`: A single compact model (e.g. `lfm2.5-350m`) serves both roles.
* **Structured Output Contracts**:
  * `LfmTaskResponse`: Strict JSON for task execution (`thought`, `tool_name`, `tool_input`, `tool_output`, `error`, `final_output`, `succeeded`, `score`).
  * `LfmProposeResponse`: Strict JSON for candidate proposals (`invariant_diffs`, `domain_diffs`).
  * `LfmComposeResponse`: Strict JSON for synthesis (`adopted_diffs`, `lesson_learned`, `winning_score`, `final_output`, `succeeded`).
* **Markdown Code Fence Sanitizer (`sanitize_json_payload`)**:
  * Strips optional ```json ... ``` or ```JSON ... ``` wrappers emitted by small models before deserialization, ensuring reliable JSON parsing.
* **System Prompt Assembly**:
  * `build_harness_system_prompt()`: Packs all 8 mounted harness components into a structured system prompt header block.

---

### 1.5. Universal File Mounter (`mounter.rs`)
* **Dual Routing Mechanism (`FileMounter::mount_file`)**:
  * `MountTarget::PromptContext`: Reads the file and stages it in memory (`StagedContextFile`), automatically injecting its filename and content into user prompts for subsequent turns.
  * `MountTarget::HarnessSlot(component)`: Reads the file and writes its content directly into the specified harness component on disk, respecting validation rules (e.g. valid JSON for `Subagents`).

---

## 2. Desktop GUI Studio: `ecoharness-gui`

The `ecoharness-gui` crate provides a native desktop studio built with `iced` 0.14, vector icons via `lucide-icons`, and native file dialogs via `rfd`.

### 2.1. Elm State Management (`state.rs`)
* **State Machine (`AppState`)**:
  * Tracks active `HarnessWorkspace`, probe directory, and `OllamaDriver`.
  * Manages multiple `ChatSession`s, active session ID, and message history.
  * Maintains discovered Ollama models, selected model, and model roles.
  * Holds editor buffers for the 8-component inspector and live experience preview.
  * Tracks execution status (`Idle`, `RunningSinglePass`, `RunningPlanModePpc`, `Error`).
* **Exhaustive Message Architecture (`Message`)**:
  * Session management: `SelectChatSession`, `CreateNewChatSession`.
  * Ollama control: `SelectOllamaModel`, `RefreshOllamaModels`, `OllamaModelsLoaded`, `AssignModelRole`, `UpdateOllamaEndpoint`, `UpdateTemperature`, `ToggleEnforceJsonSchema`.
  * 8-Component Inspector: `SelectHarnessComponent`, `EditHarnessComponentBuffer`, `SaveActiveHarnessComponent`.
  * Workbench & File Mounting: `PromptInputChanged`, `TogglePlanMode`, `SelectAddFileTarget`, `OpenNativeFileDialog`, `AttachFile`, `ClearStagedFiles`, `SendPrompt`.
  * Async Callbacks: `SinglePassFinished`, `PlanModePpcFinished`.
* **Asynchronous Task Dispatching**:
  * Non-blocking background tasks using `iced::Task::perform` for model sync, native file dialogs, single-pass LLM calls, and recursive PPC runs.
* **Lucide Icon Mapping (`AppState::icon_for`)**:
  * Mapped vector glyphs: `ChatList` (`MessageSquare`), `OllamaServer` (`Cpu`), `HarnessFiles` (`FileText`), `AddFile` (`Paperclip`), `PlanMode` (`GitBranch`), `SendPrompt` (`Send`), `RefreshModels` (`RefreshCw`), `SaveComponent` (`Save`).

---

### 2.2. Two-Pane Desktop Interface (`view.rs`)

#### Left Pane: Master-Detail Sidebar
1. **Master List A (Chat Sessions)**:
   * Displays all active sessions with indicator bullets (`● Session 1`).
   * `+ New` button to instantiate fresh sessions.
2. **Master List B (Ollama Models)**:
   * Displays discovered models with selection markers (`▸ lfm2.5-350m`).
   * `Sync` button to refresh model tags from Ollama `/api/tags`.
3. **Contextual Detail Sub-Pane**:
   * **When Chat Session is active (8-File Harness Inspector)**:
     * Component selection grid for all 8 components (`SystemPrompt`, `Guardrails`, `Memory`, `ToolGuidance`, `Subagents`, `Skills`, `Scripts`, `Plugins`).
     * Real-time file path indicator and `Save` button.
     * Text editor input bound to the active component's buffer.
     * Disk persistence verification preview.
   * **When Ollama Model is active (Model Config)**:
     * Endpoint configuration input (`http://localhost:11434`).
     * Role assignment buttons (`Set Executor`, `Set Editor`, `Set Both`).
     * Strict JSON schema toggle checkbox.
     * Active model role and model pairing summary.

#### Right Pane: Chat, Plan Mode & Add File Workbench
1. **Top Header & Status Bar**:
   * Displays active Executor model, Editor model, and real-time execution status badge.
   * `Plan Mode (Recursive PPC)` toggle switch to alternate between standard conversational chat and recursive self-improving runs.
2. **Conversation Stream**:
   * Multi-turn chat message bubbles with sender tags (`You`, `Harness² Agent`, `System`).
   * Trajectory score badges (e.g. `score: 0.95`).
   * **Plan Mode Adopted Diffs Card**: Renders component badges and rationales for diffs accepted by the Composer.
   * **Experience Lesson Callout**: Displays lessons appended to `EXPERIENCE.md`.
3. **Bottom Add File & Prompt Bar**:
   * Native OS file picker (`Add File`) via `rfd`.
   * Routing mode selectors: `Route to Prompt` (inject into LLM context) vs `Mount to [Component]` (write to component file on disk).
   * Staged files counter and `Clear` button.
   * Multi-turn prompt input box with keyboard submission on `Enter`.
   * Prominent `Send` button with Lucide paper-airplane glyph.

---

## 3. Test & Verification Matrix

The repository features comprehensive automated test suites covering both crates:

| Crate | Test Name | Purpose | Result |
| :--- | :--- | :--- | :--- |
| `ecoharness` | `test_workspace_init_creates_all_8_components_and_experience_file` | Verifies disk creation of all 8 files and `EXPERIENCE.md` with default content | **Passed** |
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
| `ecoharness` | `test_ollama_multiturn_react_loop_executes_script_and_feeds_output_back` | ReAct loop executes real script, feeds stdout back into turn 2, and scores 1.0 | **Passed** |
| `ecoharness` | `test_ollama_multiturn_loop_records_guardrail_violation_and_blocks_subprocess` | Pre-execution guardrail blocks forbidden tool and records error in step | **Passed** |
| `ecoharness` | `test_live_ollama_lfm2_5_350m_smoke_gate` | Optional live hardware test against local Ollama daemon | *Ignored (Run with `--ignored`)* |
| `ecoharness-gui` | `test_left_pane_switching_between_chat_and_ollama_updates_detail_pane` | Tests sidebar list switching between session inspector and model config | **Passed** |
| `ecoharness-gui` | `test_left_detail_pane_harness_inspector_edits_and_saves_component_to_disk` | Tests GUI buffer editing and atomic saving to disk | **Passed** |
| `ecoharness-gui` | `test_right_pane_add_file_routes_to_prompt_context_or_harness_slot` | Tests GUI file attachment routing to prompt or slot | **Passed** |
| `ecoharness-gui` | `test_right_pane_normal_chat_mode_dispatches_single_pass_execution` | Tests single-pass turn dispatch and trajectory rendering | **Passed** |
| `ecoharness-gui` | `test_right_pane_plan_mode_runs_propose_probe_compose_and_records_diffs` | Tests recursive PPC execution, diff recording, and lesson capture | **Passed** |
| `ecoharness-gui` | `test_lucide_icon_mapping_for_navigation_and_modes` | Validates Lucide vector icon mappings | **Passed** |

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

### Launch the Native Desktop Studio
```bash
cargo run -p ecoharness-gui
```
