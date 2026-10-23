# `harness2-rs` — Rust SDK & Iced Desktop Studio for Recursive Agent Harnessing

A pure-Rust implementation of Google Cloud AI Research’s **Harness²** architecture, built from the ground up using strict Test-Driven Development (TDD) and optimized for compact local models like **Liquid AI’s `LFM2.5-350M`** running on **Ollama**.

Instead of modifying model weights, `harness2-rs` treats the surrounding agent environment as **8 editable file-mounted components** and improves task performance at test time through two recursive loops:
1. **Task-Level Recursion (`Propose -> Probe -> Compose`):** Runs a reference pass, proposes orthogonal **Invariant** (planning/tool habits) and **Domain** (task-specific rules) patches, probes both concurrently in isolated filesystem sandboxes, and composes the winning diffs.
2. **Harnessing-Level Recursion (`EXPERIENCE.md`):** Persists winning lessons to disk and retrieves the Top-$K$ most relevant lessons on future tasks so small models never overflow their context window.

---

## Workspace Architecture

This Cargo workspace is split into two strictly decoupled crates:

| Crate | Path | Description |
|---|---|---|
| **`ecoharness`** | `crates/ecoharness` | Headless Rust SDK for CLI, server, or embedded agents. Zero GUI dependencies. Includes the 8-file `HarnessWorkspace`, `TrajectoryCompressor`, `OllamaDriver`, multi-turn `ScriptRunner`, `GuardrailEnforcer`, `SubagentDispatcher`, `PluginRunner`, `JsonSchemaEvaluator`, and `HarnessEngine`. |
| **`ecoharness-gui`** | `crates/ecoharness-gui` | A 2-pane desktop workbench built with `iced` (v0.14) and `lucide-icons` for interactive experimentation, live 8-file multi-line editing, session persistence, temperature sliders, and visual **Plan Mode** live stage tracking. |

> [!TIP]
> **Documentation Portal:**
> * For human engineers and deep implementation details, see [docs/README.md](file:///C:/RustProjects/harness2-rust/docs/README.md) and [docs/IMPLEMENTATION.md](file:///C:/RustProjects/harness2-rust/docs/IMPLEMENTATION.md).
> * For AI Coding Agents, check the dedicated [aidocs/](file:///C:/RustProjects/harness2-rust/aidocs/README.md) directory for machine-readable architecture maps, non-negotiable invariants, and API cheatsheets.

---

## The 8 Mounted Harness Components

Every `HarnessWorkspace` directory manages 8 canonical files alongside `EXPERIENCE.md` and `.history/` snapshots:

| Enum Variant | Relative File Path | Runtime Role |
|---|---|---|
| `SystemPrompt` | `system_prompt.md` | Core identity and output formatting instructions. |
| `Guardrails` | `guardrails.md` | Enforced pre-execution rules (`DENY_TOOL:`, `DENY_ARG_PATTERN:`, path-traversal protection). |
| `Memory` | `memory.md` | Working domain memory and schema notes. |
| `ToolGuidance` | `tool_guidance.md` | Parameter formatting rules and tool invocation rubrics. |
| `Subagents` | `subagents.json` | JSON-validated subagent definitions (atomic rollback on malformed JSON). |
| `Skills` | `skills/manifest.md` | Reusable multi-step workflow guides. |
| `Scripts` | `scripts/manifest.md` | Executable local tools spawned via `ScriptRunner` with `HARNESS_TOOL_INPUT`. |
| `Plugins` | `plugins/manifest.md` | Pre-prompt and post-response middleware configuration. |

---

## Quickstart with Ollama & `LFM2.5-350M`

### 1. Start Ollama & Pull `LFM2.5-350M`
```bash
ollama serve
ollama pull LiquidAI/lfm2.5-350m
```

### 2. Run the Full Workspace TDD Suite
```bash
# Run all 36 deterministic unit tests (0 network sockets required)
cargo test --workspace

# Run the live Ollama LFM2.5-350M hardware smoke gate
OLLAMA_MODEL="LiquidAI/lfm2.5-350m" cargo test -p ecoharness -- --ignored
```

### 3. Launch the 2-Pane Desktop Studio (`ecoharness-gui`)
```bash
cargo run -p ecoharness-gui
```

---

## Embedding `ecoharness` in Your Own Rust Project

Add `ecoharness` to your `Cargo.toml`:

```toml
[dependencies]
ecoharness = { path = "crates/ecoharness" }
tokio = { version = "1.43", features = ["full"] }
```

### Minimal SDK Usage

```rust
use ecoharness::{
    HarnessEngine, HarnessWorkspace, JsonFieldType, JsonSchemaEvaluator,
    OllamaConfig, OllamaDriver,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ws = HarnessWorkspace::init("./my_agent_harness")?;

    let evaluator = Arc::new(
        JsonSchemaEvaluator::new()
            .require_field("user_id", JsonFieldType::Integer),
    );

    let driver = OllamaDriver::new(OllamaConfig::default())
        .with_max_turns(3)
        .with_script_execution(true)
        .with_evaluator(evaluator);

    let engine = HarnessEngine::new(driver, 1200);

    // Standard single-pass execution (Plan Mode OFF)
    let trace = engine
        .run_single_pass(&ws, "Extract user_id from 'User 42 logged in'")
        .await?;
    println!("Single-pass score: {:.2}", trace.score);

    // Full recursive Propose -> Probe -> Compose loop (Plan Mode ON)
    let decision = engine
        .run_propose_probe_compose(
            &ws,
            std::path::Path::new("./tmp_probes"),
            "Extract user_id from 'User 42 logged in'",
        )
        .await?;

    println!("Winning score: {:.2}", decision.winning_trajectory.score);
    println!("Lesson recorded to EXPERIENCE.md: {}", decision.lesson_learned);
    Ok(())
}
```

---

## Using the 2-Pane `harness2-gui` Workbench

### Left Pane: Master Lists + Contextual Detail Sub-Pane
* **Chat Sessions List:** Create or switch between persisted conversations (`sessions.json`). Selecting a session switches the bottom **Detail Sub-Pane** to the **8-File Harness Inspector**, featuring a multi-line editor, instant disk save, and one-click `Checkpoint v1` / `Rollback v1` snapshot controls.
* **Ollama Models List:** Click **Sync** to query `GET /api/tags` on your Ollama server. Selecting a model switches the bottom **Detail Sub-Pane** to **Model Config**, where you can edit the server URL, toggle strict JSON schema enforcement, and assign the model role (**Executor**, **Editor**, or **Both**).

### Right Pane: Chat Stream, Add File & Plan Mode
* **Add File Routing:** Click **Route to Prompt** to stage external files into the LLM prompt context, or click **Mount to `<Component>`** to overwrite one of the 8 harness files directly from disk.
* **Plan Mode Toggle:**
  * **OFF:** Dispatches a fast single-pass `Executor` run.
  * **ON:** Runs the full **Propose -> Probe -> Compose** loop, displaying live stage progress in the header bar, committing winning diffs to disk only if they beat the reference score, and badging the assistant reply with adopted component diffs and `EXPERIENCE.md` lessons.

---

## Test-Driven Development (TDD) Coverage Matrix

| Phase | Module | Tests | Key Invariants Verified |
|---|---|---|---|
| **Phase 1** | `workspace`, `trajectory`, `engine` | 6 | 8-file init, sandbox mutation isolation, transactional JSON rollback, budget compression, concurrent probing, regression rejection. |
| **Phase 2** | `ollama`, `mounter` | 6 | `/api/tags` discovery, `/api/chat` 8-file mounting, Markdown code-fence sanitization, prompt-context vs. harness-slot file routing, live smoke gate. |
| **Phase 3** | `harness2-gui` (`state`, `view`) | 6 | Headless Elm `AppState::update` transitions for Master-Detail switching, inspector saving, file staging, Normal vs. Plan Mode routing, Lucide icon mapping. |
| **Phase 4A** | `guardrails`, `scripts`, `evaluator` | 5 | Pre-execution tool/arg blocking & path-traversal protection, subprocess execution with `HARNESS_TOOL_INPUT`, objective JSON schema grading, multi-turn ReAct loop. |
| **Phase 4B** | `workspace`, `engine`, `harness2-gui` | 7 | Top-$K$ keyword experience retrieval, `.history` versioned snapshots & rollback, ordered `PpcStageEvent` streaming, multi-line `text_editor::Action`, `sessions.json` persistence. |

---

## Attribution & Disclaimer

* **Inspiration:** This project is an independent, clean-room Rust implementation inspired by the architecture proposed in Google Cloud AI Research’s published Harness² research.
* **Disclaimer:** This project is independent community software and is **not** affiliated with, endorsed by, or sponsored by Google LLC or Alphabet Inc. All trademarks belong to their respective holders.

---

## License

This project is licensed under the [MIT License](file:///C:/RustProjects/harness2-rust/LICENSE).
