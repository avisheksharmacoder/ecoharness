# EcoHarness Usage & Workflows Guide

This guide explains how to use **EcoHarness** in practice, covering both the **Native Desktop Studio GUI** (`ecoharness-gui`) and the **Headless Rust SDK** (`ecoharness`).

---

## 1. Using the Native Desktop Studio (`ecoharness-gui`)

The Desktop Studio is a 2-pane reactive environment built with `iced` (0.14) and vector icons (`lucide-icons`).

### Launching the Studio
```bash
cargo run -p ecoharness-gui
```
*(By default, the GUI automatically initializes a persistent workspace in your operating system's temporary/data directory and restores prior sessions from `sessions.json` if available).*

---

### Interface Anatomy

```text
┌────────────────────────────────────────┬────────────────────────────────────────────────────────┐
│  LEFT PANE (Master Lists & Inspector)  │  RIGHT PANE (Execution & Chat Studio)                  │
├────────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 💬 Chat Sessions                       │ ⚙️ Model: [llama3.2:latest ▼]  Status: Ready  [Plan Mode]│
│   ● Session 1                          ├────────────────────────────────────────────────────────┤
│   + New Session                        │ 💬 Chat & Probe Stream:                                │
│                                        │   You: Process invoice_8842.json                       │
│ 🤖 Ollama Models [Sync]                │   Harness² Agent: Loaded invoice. Charged 4250 cents.  │
│   ★ llama3.2:latest   [Active]         │   System: Adopted 1 winning diff to Tool Guidance.     │
│   ▸ qwen2.5:0.5b      [Use]            ├────────────────────────────────────────────────────────┤
│                                        │ 📎 [Prompt Context ▼] [Attach File]                    │
│ 📄 8-File Harness Inspector            │ ┌────────────────────────────────────────────────────┐ │
│   [SystemPrompt] [Guardrails] [Memory] │ │ Type a task or instruction...                      │ │
│   [ToolGuidance] [Subagents]  [Skills] │ └────────────────────────────────────────────────────┘ │
│   [Scripts]      [Plugins]             │                                            [Send Prompt]
│   File: tool_guidance.md  [Save]       │                                                        │
│   [Checkpoint v1]  [Rollback v1]       │                                                        │
│   ┌──────────────────────────────────┐ │                                                        │
│   │ Multi-line text editor buffer    │ │                                                        │
│   └──────────────────────────────────┘ │                                                        │
│   Temp: 0.10 | Scripts: ON | MaxTurns: 3│                                                        │
└────────────────────────────────────────┴────────────────────────────────────────────────────────┘
```

### Left Pane: Master Controls & 8-File Inspector
1. **Chat Sessions List:**
   * Click any session to switch conversations.
   * Click **`+`** to spawn a fresh session. Sessions automatically save to `sessions.json` on disk.
2. **Ollama Models List:**
   * Polls your local or Dockerized Ollama daemon in real time (every 3 seconds).
   * Shows `★` and `[Active]` for the currently running model.
   * Click **`Use`** on any detected model to immediately make it the active model for execution and editing.
   * Click the model title to view and adjust model parameters in the detail subpane below.
   * Click **`Sync`** to force an instant refresh from Ollama.
3. **8-File Harness Inspector & Multi-Line Editor:**
   * Click any of the 8 file buttons (`SystemPrompt`, `Guardrails`, `Memory`, `ToolGuidance`, `Subagents`, `Skills`, `Scripts`, `Plugins`) to load its live contents into the multi-line editor.
   * Edit the text directly in the editor buffer and click **`Save`** to write updates directly to the on-disk workspace.
   * **Version Checkpoints:** Click **`Checkpoint v1`** to create an immutable snapshot in `.history/v1.json`. Click **`Rollback v1`** at any time to instantly revert the workspace.
   * **Telemetry & Sliders:** Adjust model generation temperature (from `0.0` to `2.0`), toggle JSON schema enforcement, and toggle multi-turn scripts execution (`ON`/`OFF`).

### Right Pane: Interaction & PPC Studio
1. **Top Header Bar:**
   * **Active Model Dropdown:** Switch the active model on the fly using the pick-list dropdown.
   * **Plan Mode Toggler:** Toggle between **Single-Pass Execution** (Plan Mode OFF) and **Recursive PPC Optimization** (Plan Mode ON).
   * **Status Badge:** Real-time indicator displaying engine progress (e.g. `Reference Started`, `Candidates Proposed`, `Probes Completed`, `Composed`).
2. **Chat & Plan Mode Stream:**
   * Displays prompt inputs, thoughts, tool invocation traces, final model answers, deterministic task scores, and adopted diff summaries.
3. **Add File Bar:**
   * Select routing target:
     * **`Prompt Context`**: Attaches an external document (e.g., PDF text, JSON dataset, code file) directly into the model's user prompt context.
     * **`Harness Slot`**: Overwrites one of the 8 harness components with the chosen file.
   * Click **`Attach File`** to pick any local file via the native system file dialog.
4. **Composer Input:**
   * Type your task prompt and press **`Send Prompt`** (or Enter).

---

## 2. Using the Headless Rust SDK (`ecoharness`)

The headless SDK is completely decoupled from any GUI libraries and can be embedded in CLI tools, backend microservices, or headless benchmarking pipelines.

### Adding EcoHarness to `Cargo.toml`
```toml
[dependencies]
ecoharness = { path = "path/to/ecoharness/crates/ecoharness" }
tokio = { version = "1.0", features = ["full"] }
```

### Initializing a Workspace
```rust
use ecoharness::{HarnessWorkspace, HarnessComponent};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_path = Path::new("./my_agent_workspace");

    // Initialize all 8 canonical files + EXPERIENCE.md
    let workspace = if workspace_path.join("EXPERIENCE.md").exists() {
        HarnessWorkspace::load(workspace_path)?
    } else {
        HarnessWorkspace::init(workspace_path)?
    };

    println!("Loaded workspace at {:?}", workspace.root());
    Ok(())
}
```

### Configuring the Driver & Connecting to Ollama
```rust
use ecoharness::{OllamaConfig, OllamaDriver};

// Defaults automatically check OLLAMA_BASE_URL, OLLAMA_HOST, and OLLAMA_MODEL
let config = OllamaConfig {
    base_url: "http://localhost:11434".to_string(),
    executor_model: "llama3.2".to_string(),
    editor_model: "llama3.2".to_string(),
    temperature: 0.1,
    enforce_json_schema: true,
};

let driver = OllamaDriver::new(config)
    .with_max_turns(3)
    .with_script_execution(true)
    .with_subagents(true)
    .with_plugins(true);
```

### Single-Pass Execution (Plan Mode OFF)
```rust
use ecoharness::HarnessEngine;

let engine = HarnessEngine::new(driver, 1200);

let trajectory = engine
    .run_single_pass(&workspace, "Calculate total sales from invoices.")
    .await?;

println!("Task Succeeded: {}", trajectory.succeeded);
println!("Score: {:.2}", trajectory.score);
println!("Final Output: {}", trajectory.final_output.unwrap_or_default());
```

### Recursive PPC Execution (Plan Mode ON)
```rust
use ecoharness::HarnessEngine;
use std::path::Path;

let probe_sandbox_dir = Path::new("./sandboxes");

let engine = HarnessEngine::new(driver, 1200)
    .with_cleanup_sandboxes(true); // Removes probe_invariant/probe_domain after run

let decision = engine
    .run_propose_probe_compose_with_progress(
        &workspace,
        probe_sandbox_dir,
        "Verify customer invoice #8842 and charge payment.",
        |stage_event| {
            println!("Live stage: {:?}", stage_event);
        },
    )
    .await?;

println!("Adopted Diffs: {}", decision.adopted_diffs.len());
println!("Lesson Learned: {}", decision.lesson_learned);
println!("Winning Score: {:.2}", decision.winning_trajectory.score);
```

---

## 3. Core Operational Workflows

### Workflow 1: Registering Local Executable Tools
1. In `scripts/manifest.md`, define executable tools:
   ````markdown
   # Scripts Manifest
   ```json
   {
     "tools": [
       {
         "name": "calc",
         "description": "Performs mathematical arithmetic",
         "program": "python3",
         "args": ["-c", "import os; print(eval(os.environ['HARNESS_TOOL_INPUT']))"]
       }
     ]
   }
   ```
   ````
2. The agent can invoke `"calc"` with `{"tool_name": "calc", "tool_input": "25 * 40"}`.
3. The runner passes the input through `HARNESS_TOOL_INPUT` securely without shell concatenation.

### Workflow 2: Restricting Execution with Guardrails
In `guardrails.md`, specify safety directives:
```markdown
# Guardrails Policy
- DENY_TOOL: format_disk
- DENY_TOOL: raw_bash
- DENY_ARG_PATTERN: rm -rf
- DENY_ARG_PATTERN: DROP TABLE
```
* If the agent attempts to invoke `format_disk` or inject `rm -rf`, `GuardrailEnforcer` intercepts the call **before subprocess creation** and returns an error message in the trajectory.

### Workflow 3: Orchestrating Subagents
In `subagents.json`, configure task-specialized models:
```json
{
  "subagents": [
    {
      "name": "code_auditor",
      "model": "qwen2.5-coder:7b",
      "system_prompt": "You are a senior security engineer. Analyze the code strictly for memory safety."
    }
  ]
}
```
* The main agent calls `subagent:code_auditor` with the code payload, and the dispatcher routes the turn to the specialized model.
