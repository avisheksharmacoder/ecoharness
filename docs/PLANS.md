# System Architecture Plans & Implementation Status

This document catalogs the implementation roadmap for the **Harness²** system, tracking progress from the Phase 1–3 foundation through the complete **Phase 4A, Phase 4B, and Phase 5 Green** milestones. For an in-depth technical analysis of data structures and algorithms, see [docs/IMPLEMENTATION.md](file:///C:/RustProjects/harness2-rust/docs/IMPLEMENTATION.md).

---

## Executive Implementation Summary

Every planned subsystem has been fully implemented, integrated, and verified green under strict test-driven development:

| Subsystem | Original Phase 1–3 Baseline | Final Phase 5 Green Implementation | Status |
| :--- | :--- | :--- | :--- |
| **Tool Execution** | Single-turn simulated JSON responses | Multi-turn ReAct loop with real `ScriptRunner` (`tokio::process::Command`), environment variable injection, and output feedback | **100% Implemented & Tested** |
| **Scoring & Verification** | Self-reported LLM `score` and `succeeded` | Deterministic `TaskEvaluator` and `JsonSchemaEvaluator` enforcing required keys and strict integer/string types | **100% Implemented & Tested** |
| **Safety & Middleware** | Passive prompt injection + basic JSON parse | Active `GuardrailEnforcer`, `SubagentDispatcher` (model delegation), and `PluginRunner` (<think> stripping + comma repair) | **100% Implemented & Tested** |
| **Experience Memory** | Full-file read & append in `EXPERIENCE.md` | Top-$K$ keyword relevance retrieval + `.history/<tag>.json` versioned harness snapshot rollback | **100% Implemented & Tested** |
| **PPC Orchestration** | Ephemeral directories retained on disk | 5-stage ordered event streaming (`PpcStageEvent`), automatic rollback checkpoints, and automatic sandbox cleanup | **100% Implemented & Tested** |
| **Desktop Studio** | Single-line inspector input, monolithic task | True multi-line `text_editor::Content`, live stage tracking, temperature slider, and on-disk session persistence (`sessions.json`) | **100% Implemented & Tested** |

---

## 1. Multi-Turn Tool & Script Execution (`ScriptRunner`)

### Completed Architecture
* **`ScriptRunner` (`crates/ecoharness/src/scripts.rs`)**:
  * Parses JSON tool manifests embedded in `scripts/manifest.md`.
  * Spawns sandboxed child processes using `tokio::process::Command`.
  * Injects tool parameters via `HARNESS_TOOL_INPUT` environment variables.
  * Captures `stdout`, `stderr`, and exit codes with execution error handling.
* **Multi-Turn ReAct Loop (`crates/ecoharness/src/ollama.rs`)**:
  * Configurable turn limits via `driver.with_max_turns(usize)`.
  * Automatically intercepts tool calls, verifies them against `GuardrailEnforcer`, runs `ScriptRunner`, appends execution observations to `TrajectoryStep`, and feeds observations back into the next model turn.

---

## 2. Objective External Scoring (`TaskEvaluator`)

### Completed Architecture
* **`TaskEvaluator` Trait (`crates/ecoharness/src/evaluator.rs`)**:
  * Decouples evaluation from self-reported LLM scores.
  * Overrides hallucinated scores before the PPC regression gate commits diffs.
* **`JsonSchemaEvaluator`**:
  * Enforces required field existence and typed schema validation (`String`, `Integer`, `Float`, `Boolean`, `Array`, `Object`).
  * Downgrades invalid trajectories (e.g. string formatted amounts when integer cents are required) and records diagnostic errors in step traces.

---

## 3. Active Runtime Enforcement: Guardrails, Subagents & Plugins

### Completed Architecture
* **`GuardrailEnforcer` (`crates/ecoharness/src/guardrails.rs`)**:
  * Enforces security policies before subprocess spawning or subagent delegation.
  * Blocks denied tools (`DENY_TOOL`), forbidden argument patterns (`DENY_ARG_PATTERN`), and path traversal (`../`).
* **`SubagentDispatcher` (`crates/ecoharness/src/subagents.rs`)**:
  * Parses `subagents.json` into typed `SubagentSpec` definitions.
  * Resolves `subagent:<name>` tool calls and delegates execution to the subagent's designated model with its isolated system prompt.
  * Keeps private subagent prompts out of the parent executor's system prompt, eliminating prompt pollution.
* **`PluginRunner` (`crates/ecoharness/src/plugins.rs`)**:
  * Parses `plugins/manifest.md` directives.
  * Truncates overlong user prompts using `MAX_PROMPT_CHARS`.
  * Strips `<think>...</think>` tags emitted by reasoning models.
  * Repairs trailing JSON commas (`,}` or `,]`) outside string literals to ensure clean `serde_json` deserialization on compact models like `LFM2.5-350M`.

---

## 4. Scalable `EXPERIENCE.md` Retrieval & Snapshot Versioning

### Completed Architecture
* **Top-$K$ Experience Retrieval (`crates/ecoharness/src/workspace.rs`)**:
  * Implements in-memory keyword relevance scoring with English stopword pruning.
  * Injects only the top-$K$ (default 3) most relevant lessons into the Proposer prompt, preserving token headroom.
* **Harness Snapshot VCS (`.history/<tag>.json`)**:
  * `save_versioned_snapshot`: Saves timestamped JSON snapshots of all 8 harness files into the `.history/` directory.
  * `rollback_to_snapshot`: Atomically rolls back the live base workspace to any named historical checkpoint.
  * `pre_ppc_auto`: Automatically saved before every composed patch commit.

---

## 5. Ephemeral Probe Sandbox Cleanup

### Completed Architecture
* **`HarnessEngine::with_cleanup_sandboxes` (`crates/ecoharness/src/engine.rs`)**:
  * Configures whether ephemeral probe sandboxes are purged upon PPC completion.
* **`HarnessEngine::cleanup_probe_dirs`**:
  * Programmatically removes `probe_invariant` and `probe_domain` directories from `probe_root` after composition finishes, preventing disk bloat.

---

## 6. Desktop Studio Production Features (`ecoharness-gui`)

### Completed Architecture
* **Startup Session Restoration (`crates/ecoharness-gui/src/state.rs`)**:
  * `AppState::new_with_startup_restore`: Auto-loads `<workspace_root>/sessions.json` on app boot and initializes the driver with multi-turn scripts (3 turns), subagents, and plugins enabled.
* **True Multi-Line Editor (`crates/ecoharness-gui/src/view.rs`)**:
  * `iced::widget::text_editor::Content` replaces single-line inputs with a full multi-line editor supporting cursor navigation, selection, and copy-paste buffer synchronization.
* **Diagnostic Telemetry Header**:
  * `inspector_summary`: Renders live temperature, script status, max turn count, and top lesson line in the 8-File Harness Inspector header.
* **Fine-Grained Controls**:
  * Temperature slider (`0.0`–`2.0`, step `0.05`).
  * Snapshot save/rollback buttons (`Checkpoint v1` / `Rollback v1`).
  * Live stage progress tracking via `Message::ApplyLivePpcRun`.

---

## Verification Summary Across All Phases

| Phase | Focus Area | SDK Tests | GUI Tests | Status |
| :--- | :--- | :---: | :---: | :--- |
| **Phase 1** | 8-Component Workspace & Sandboxing | 6 | — | **Green** |
| **Phase 2** | Ollama Driver, Trajectory Compressor & FileMounter | 6 | 6 | **Green** |
| **Phase 3** | Plan Mode PPC Benchmark & Self-Healing | 1 | — | **Green** |
| **Phase 4A** | Guardrails, Scripts, Evaluator & ReAct | 5 | — | **Green** |
| **Phase 4B** | Top-K Experience, Snapshot VCS & Live PPC | 3 | 4 | **Green** |
| **Phase 5** | Subagents, Plugins, Sandbox Cleanup & Startup Restore | 3 | 3 | **Green** |
| **Totals** | **Complete Workspace Verification** | **24 (23 + 1 ign)** | **13** | **36 Passed (1 Ignored)** |
