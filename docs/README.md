# EcoHarness / Harness² Documentation Index

Welcome to the comprehensive technical documentation for **EcoHarness** (`harness2-rust`), a pure-Rust clean-room implementation of Google Cloud AI Research's **Harness²** architecture, optimized for compact local language models such as Liquid AI's `LFM2.5-350M` running on Ollama.

---

## Documentation Navigation

| Document | Description | Target Audience |
| :--- | :--- | :--- |
| **[docs/HARNESS_SPECIFICATION.md](file:///C:/RustProjects/harness2-rust/docs/HARNESS_SPECIFICATION.md)** | **Harness Specification & Authoring Guide:** Exact file formats, syntax rules, schemas, and directives for all 8 canonical harness files. | Developers, Agent Authors |
| **[docs/IMPLEMENTATION.md](file:///C:/RustProjects/harness2-rust/docs/IMPLEMENTATION.md)** | **Deep Dive Implementation Guide:** Complete architectural details of how all new features are implemented, including `SubagentDispatcher`, `PluginRunner`, sandbox cleanup, startup restoration, and GUI widgets. | Developers, Architects, Contributors |
| **[docs/FEATURES.md](file:///C:/RustProjects/harness2-rust/docs/FEATURES.md)** | **System Feature Matrix:** Full inventory of all 8 harness components, SDK APIs, GUI capabilities, and 36-test verification matrix. | Users, Integrators, Evaluators |
| **[docs/PLANS.md](file:///C:/RustProjects/harness2-rust/docs/PLANS.md)** | **Roadmap & Phase Status:** Chronological record of Phase 1 through Phase 5 Green milestones and test status. | Engineering Management, Reviewers |
| **[aidocs/README.md](file:///C:/RustProjects/harness2-rust/aidocs/README.md)** | **AI Cockpit & Cognitive References:** Machine-readable architecture maps, non-negotiable invariants, and fast tool playbooks. | AI Coding Assistants, LLMs |

---

## Workspace Crates Overview

The workspace is strictly partitioned into two decoupled crates:

1. **[`crates/ecoharness`](file:///C:/RustProjects/harness2-rust/crates/ecoharness)** (`harness2`):
   * Headless Rust SDK with **zero GUI, windowing, or Iced dependencies**.
   * Provides the 8-component file-mounted [`HarnessWorkspace`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/workspace.rs), multi-turn [`OllamaDriver`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/ollama.rs), recursive [`HarnessEngine`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/engine.rs), [`GuardrailEnforcer`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/guardrails.rs), [`ScriptRunner`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/scripts.rs), [`SubagentDispatcher`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/subagents.rs), [`PluginRunner`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/plugins.rs), and [`JsonSchemaEvaluator`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/evaluator.rs).
2. **[`crates/ecoharness-gui`](file:///C:/RustProjects/harness2-rust/crates/ecoharness-gui)** (`harness2-gui`):
   * Native 2-pane desktop studio built with `iced` 0.14, vector icons via `lucide-icons`, and native file dialogs via `rfd`.
   * Provides multi-line [`text_editor::Content`](file:///C:/RustProjects/harness2-rust/crates/ecoharness-gui/src/state.rs) editing, startup session restoration, temperature sliders, diagnostic telemetry, snapshot rollback, and live PPC stage event tracking.

---

## Verification & Test Suite

All 36 unit tests pass completely offline without network sockets via `wiremock`:

```bash
# Run the entire test suite across all crates (36 passed, 1 ignored)
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
