# Core Data Structures & APIs

A quick reference for types, enums, structs, and methods across `ecoharness` and `ecoharness-gui`.

---

## 1. Workspace (`ecoharness::workspace`)

### `HarnessComponent` (Enum)
```rust
pub enum HarnessComponent {
    SystemPrompt,  // "system_prompt.md"
    Guardrails,    // "guardrails.md"
    Memory,        // "memory.md"
    ToolGuidance,  // "tool_guidance.md"
    Subagents,     // "subagents.json"
    Skills,        // "skills/manifest.md"
    Scripts,       // "scripts/manifest.md"
    Plugins,       // "plugins/manifest.md"
}
```
* `HarnessComponent::all() -> &'static [HarnessComponent; 8]`
* `component.relative_path() -> PathBuf`
* `component.default_content() -> &'static str`

### `HarnessWorkspace` (Struct)
* `init(root: &Path) -> Result<Self, HarnessError>`: Creates 8 files + `EXPERIENCE.md`.
* `load(root: &Path) -> Result<Self, HarnessError>`: Verifies all 8 exist + parses `subagents.json`.
* `read_component(comp: HarnessComponent) -> Result<String, HarnessError>`
* `write_component(comp: HarnessComponent, content: &str) -> Result<(), HarnessError>`
* `apply_patch(patch: &HarnessPatch) -> Result<(), HarnessError>`: Atomic update with rollback.
* `snapshot() -> Result<HarnessSnapshot, HarnessError>`: In-memory snapshot.
* `clone_to_sandbox(dest: &Path) -> Result<Self, HarnessError>`: Deep disk copy for isolated probing.
* `save_versioned_snapshot(tag: &str) -> Result<(), HarnessError>`: Saves to `.history/<tag>.json`.
* `rollback_to_snapshot(tag: &str) -> Result<(), HarnessError>`: Restores from `.history/<tag>.json`.
* `retrieve_relevant_experience(query: &str, k: usize) -> Result<Vec<ExperienceEntry>, HarnessError>`: Top-$K$ keyword retrieval.
* `append_experience(lesson: &str) -> Result<(), HarnessError>`

---

## 2. Engine & PPC Loop (`ecoharness::engine`)

### `HarnessEngine<D: ExecutionDriver>` (Struct)
* `new(driver: D, max_chars_budget: usize) -> Self`
* `run_single_pass(&self, ws: &HarnessWorkspace, prompt: &str) -> Result<Trajectory, HarnessError>`
* `run_propose_probe_compose(&self, ws: &HarnessWorkspace, probe_dir: &Path, prompt: &str) -> Result<CompositionDecision, HarnessError>`
* `run_propose_probe_compose_with_progress(&self, ws: &HarnessWorkspace, probe_dir: &Path, prompt: &str, on_progress: impl Fn(PpcStageEvent)) -> Result<CompositionDecision, HarnessError>`

### `PpcStageEvent` (Enum)
```rust
pub enum PpcStageEvent {
    ReferenceStarted,
    ReferenceCompleted { score: f64, succeeded: bool },
    CandidatesProposed { invariant_diff_count: usize, domain_diff_count: usize },
    ProbesCompleted { invariant_score: f64, domain_score: f64 },
    Composed { adopted_diff_count: usize, winning_score: f64 },
}
```

### `CompositionDecision` (Struct)
```rust
pub struct CompositionDecision {
    pub winning_trajectory: Trajectory,
    pub adopted_diffs: Vec<ComponentDiff>,
    pub rejected_diffs: Vec<ComponentDiff>,
    pub lesson_learned: String,
    pub snapshot: HarnessSnapshot,
}
```

---

## 3. Evaluation & Guardrails (`ecoharness::evaluator`, `guardrails`, `scripts`)

### `JsonSchemaEvaluator`
```rust
let evaluator = Arc::new(
    JsonSchemaEvaluator::new()
        .require_field("customer_id", JsonFieldType::String)
        .require_field("amount_cents", JsonFieldType::Integer)
        .require_field("currency", JsonFieldType::String),
);
```
* Evaluates raw JSON strings against required schema and types.
* Returns `EvaluationVerdict { is_valid: bool, score: f64, reason: String }`.

### `GuardrailEnforcer`
* Parses `guardrails.md` for `DENY_TOOL:` and `DENY_ARG_PATTERN:`.
* Checks parameters for path traversal attacks (`../` or `..\`).
* `validate_tool_call(tool: &str, args: &serde_json::Value) -> Result<(), HarnessError>`

### `ScriptRunner`
* Parses `scripts/manifest.md` for registered tools.
* Executes tool program via `std::process::Command` with input serialized into `HARNESS_TOOL_INPUT`.

### `SubagentDispatcher` (`ecoharness::subagents`)
* Parses `subagents.json` into typed `SubagentManifest` containing `Vec<SubagentSpec>` (`name`, `model`, `system_prompt`).
* `resolve_subagent(ws, tool_name)` resolves `subagent:<name>` calls and dispatches to isolated subagent model with private system prompt.
* Maintains strict prompt isolation so private subagent instructions never leak into the parent executor's prompt context.

### `PluginRunner` (`ecoharness::plugins`)
* Parses `plugins/manifest.md` for active middleware directives (`ENABLE_PLUGIN: repair_trailing_commas`, `ENABLE_PLUGIN: strip_thinking_tags`, `MAX_PROMPT_CHARS: <usize>`).
* `apply_pre_prompt`: Enforces character budget constraints before tokenization.
* `apply_post_response`: Strips reasoning `<think>...</think>` tags and repairs invalid trailing commas (`,}` and `,]`) outside string literals.

---

## 4. File Routing (`ecoharness::mounter`)

### `MountTarget` (Enum)
* `PromptContext`: Stages external file into dynamic LLM prompt context (leaves harness intact).
* `HarnessSlot(HarnessComponent)`: Overwrites one of the 8 canonical harness component files directly.

### `FileMounter::mount_file(ws, driver, path, target)`
* Automatically validates JSON if target is `HarnessComponent::Subagents`.
* Returns `MountOutcome::ContextStaged` or `MountOutcome::HarnessComponentMounted`.

---

## 5. GUI State Machine (`ecoharness-gui::state`)

* **Elm Architecture Pattern:** `AppState::new(...)`, `AppState::update(&mut self, msg: Message) -> iced::Task<Message>`, `view::view(&AppState) -> Element<Message>`.
* **Detail Sub-Pane Views:**
  * `DetailPaneView::HarnessInspector { session_id, selected_component }`
  * `DetailPaneView::OllamaModelDetail { model_name, assigned_role }`
* **Session Persistence:**
  * Auto-saves to `sessions.json` on new session or prompt sent.
  * Restores active sessions seamlessly on startup.
