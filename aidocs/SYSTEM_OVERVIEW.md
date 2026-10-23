# System Overview

This document describes the structure and execution flow of the `harness2-rust` repository.

---

## 1. Workspace Layout

The repository is structured as a two-crate Cargo workspace:

```text
harness2-rust/
├── Cargo.toml                  # Workspace manifest with shared dependencies
├── Cargo.lock
├── LICENSE                     # MIT License
├── README.md                   # Public documentation & architecture overview
├── aidocs/                     # Documentation dedicated to AI agents (this folder)
├── docs/                       # Detailed historical feature specifications
└── crates/
    ├── ecoharness/             # Headless Rust SDK (also known as harness2)
    │   ├── Cargo.toml
    │   ├── examples/           # invoice_agent.rs developer CLI demo
    │   └── src/
    │       ├── lib.rs          # Re-exports, error enums, and EcoHarness aliases
    │       ├── workspace.rs    # 8-file HarnessWorkspace, snapshots & EXPERIENCE.md
    │       ├── trajectory.rs   # Turn logging & strict token/char budget compressor
    │       ├── engine.rs       # Propose-Probe-Compose (PPC) recursive orchestrator
    │       ├── mounter.rs      # FileMounter (PromptContext vs HarnessSlot)
    │       ├── ollama.rs       # OllamaDriver with LFM2.5-350M JSON schema recovery
    │       ├── guardrails.rs   # Pre-execution tool & path-traversal enforcer
    │       ├── scripts.rs      # Subprocess ScriptRunner with HARNESS_TOOL_INPUT
    │       ├── evaluator.rs    # Deterministic JsonSchemaEvaluator
    │       └── tests.rs        # Deterministic SDK test suite (WireMock)
    └── ecoharness-gui/         # 2-Pane Iced Desktop Studio (also known as harness2-gui)
        ├── Cargo.toml
        └── src/
            ├── main.rs         # Iced application entrypoint (loads Lucide font)
            ├── state.rs        # Elm AppState, Message enum, session persistence
            ├── view.rs         # Responsive 2-pane view with text_editor
            └── tests.rs        # Headless Elm update tests
```

---

## 2. Core Operational Loops

### A. Task-Level Recursion: Propose $\rightarrow$ Probe $\rightarrow$ Compose (PPC)
When executing in **Plan Mode** (`HarnessEngine::run_propose_probe_compose_with_progress`):
1. **Reference Pass:** Runs an initial reference trajectory against the active base harness. Evaluates objective score $S_{\text{ref}}$.
2. **Candidates Proposed:** The model (via `OllamaDriver::propose_candidates`) proposes two orthogonal patch bundles:
   * **Invariant Candidate:** Improvements to procedural tool habits, planning steps, or error rubrics (`tool_guidance.md`, `guardrails.md`, `skills/`).
   * **Domain Candidate:** Task-specific rules, schema hints, or domain facts (`memory.md`, `system_prompt.md`).
3. **Concurrent Probing:** Two isolated sandboxes (`base_dir/probes/invariant` and `base_dir/probes/domain`) are created via `HarnessWorkspace::clone_to_sandbox`. Both candidates are probed simultaneously using `tokio::join!`.
4. **Composition:**
   * If either candidate improves upon $S_{\text{ref}}$, non-conflicting winning component diffs are composed.
   * If both candidates regress ($S < S_{\text{ref}}$), **all diffs are discarded** to prevent regression.
5. **Persistence & Snapshotting:**
   * An automatic snapshot (`"pre_ppc_auto"`) is written to `.history/`.
   * Winning diffs are committed to the base harness.
   * The distilled lesson learned is written to `EXPERIENCE.md`.

---

### B. Harnessing-Level Recursion: Top-$K$ Experience Retrieval
* Compact models like `LFM2.5-350M` have small context windows (~2,048 tokens).
* `EXPERIENCE.md` stores markdown bullet points recording past lessons.
* `HarnessWorkspace::retrieve_relevant_experience(task_prompt, k)` tokenizes the prompt, filters out English stop words, scores each lesson by keyword intersection, and injects only the Top-$K$ (default: Top-3) most relevant lessons into prompt context.

---

### C. Multi-Turn ReAct Loop with Guardrails
When executing tools:
1. Model returns a JSON tool call (e.g. `{"tool_name": "...", "parameters": {...}}`).
2. **Guardrails Check:** `GuardrailEnforcer::validate_tool_call` checks `guardrails.md` rules (`DENY_TOOL:`, `DENY_ARG_PATTERN:`, path traversal `../`).
   * If denied, tool invocation is blocked immediately; error message is fed back to LLM context without executing subprocess.
3. **Script Execution:** If permitted, `ScriptRunner::execute_script` runs the registered tool with input passed via the environment variable `HARNESS_TOOL_INPUT`.
4. **Output Feeding:** Subprocess output (stdout/stderr) is fed back into chat turns up to `max_turns`.
5. **Grading:** Final output is graded by `JsonSchemaEvaluator` to compute an objective score $[0.0, 1.0]$.
