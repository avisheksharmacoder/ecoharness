# EcoHarness / Harness² Documentation Index

Welcome to the comprehensive technical documentation for **EcoHarness** (`harness2-rust`), a pure-Rust clean-room implementation of Google Cloud AI Research's **Harness²** architecture, engineered for local open models running via Ollama.

---

## Documentation Navigation

| Document | Description | Target Audience |
| :--- | :--- | :--- |
| **[docs/starter/README.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/starter/README.md)** | **Starter Guide & Step-by-Step Tutorial:** Zero-to-hero walkthrough covering Docker setup, first run, PPC Plan Mode, custom tools, and guardrails. | New Users, Quickstart |
| **[docs/starter/sdk_quickstart.rs](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/starter/sdk_quickstart.rs)** | **SDK Code Quickstart:** Standalone, annotated Rust code example demonstrating programmatic workspace initialization, file mounting, and PPC runs. | Rust Developers |
| **[docs/starter/workspace_template/](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/starter/workspace_template)** | **Starter Workspace Template:** Ready-to-copy 8-file directory structure with baseline configurations and experience log. | All Users |
| **[docs/ARCHITECTURE_AND_CONCEPTS.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/ARCHITECTURE_AND_CONCEPTS.md)** | **Architecture & Theoretical Foundations:** Deep dive into the 8-component philosophy, PPC recursion, experience ranking, and subsystem decoupling. | Architects, Contributors |
| **[docs/USAGE_GUIDE.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/USAGE_GUIDE.md)** | **Usage & Workflows Guide:** Comprehensive instructions for both the Native Desktop Studio GUI (`ecoharness-gui`) and Headless SDK (`ecoharness`). | End Users, Developers |
| **[docs/MODEL_GUIDE.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/MODEL_GUIDE.md)** | **Open Model Deployment Guide:** Sizing taxonomy, parameter tuning, role assignment (Executor vs. Editor), and asymmetric multi-model pairing. | AI Engineers, Operators |
| **[docs/HARNESS_SPECIFICATION.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/HARNESS_SPECIFICATION.md)** | **Harness Specification & Authoring Guide:** Syntax rules, schemas, and directives for all 8 canonical harness files. | Agent Authors |
| **[docs/IMPLEMENTATION.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/IMPLEMENTATION.md)** | **Deep Dive Implementation Guide:** Complete internal design details of subagents, plugins, sandboxes, and GUI widgets. | Contributors, Reviewers |
| **[docs/FEATURES.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/FEATURES.md)** | **System Feature Matrix:** Comprehensive inventory of all components, SDK APIs, and verification matrices. | Evaluators, Integrators |
| **[docs/PLANS.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/PLANS.md)** | **Roadmap & Milestone Status:** Phase 1 through Phase 5 Green milestones. | Engineering Management |
| **[aidocs/README.md](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/aidocs/README.md)** | **AI Cockpit & Cognitive Acceleration:** High-density architecture maps, non-negotiable invariants, and symbol lines for AI assistants. | AI Pair Programmers |

---

## Workspace Crates Overview

The workspace is strictly partitioned into two decoupled crates:

1. **[`crates/ecoharness`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness)** (`harness2`):
   * Headless Rust SDK with **zero GUI, windowing, or Iced dependencies**.
   * Provides the 8-component file-mounted [`HarnessWorkspace`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/workspace.rs), multi-turn [`OllamaDriver`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/ollama.rs), recursive [`HarnessEngine`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/engine.rs), [`GuardrailEnforcer`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/guardrails.rs), [`ScriptRunner`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/scripts.rs), [`SubagentDispatcher`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/subagents.rs), [`PluginRunner`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/plugins.rs), and [`JsonSchemaEvaluator`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/evaluator.rs).
2. **[`crates/ecoharness-gui`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness-gui)** (`harness2-gui`):
   * Native 2-pane desktop studio built with `iced` 0.14, vector icons via `lucide-icons`, and native file dialogs via `rfd`.
   * Provides real-time Ollama model discovery and dynamic switching, multi-line [`text_editor::Content`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness-gui/src/state.rs) editing, startup session restoration, temperature sliders, diagnostic telemetry, snapshot rollback, and live PPC stage event tracking.

---

## Verification & Test Suite

All 36 unit tests pass completely offline without network sockets via `wiremock`:

```bash
# Run the entire test suite across all crates (36 passed, 1 ignored live test)
cargo test --workspace

# Run core SDK unit tests only (23 passed)
cargo test -p ecoharness

# Run desktop GUI headless tests only (13 passed)
cargo test -p ecoharness-gui

# Run compile and lint checks with zero warnings
cargo check --workspace --all-targets

# Launch the desktop studio
cargo run -p ecoharness-gui
```
