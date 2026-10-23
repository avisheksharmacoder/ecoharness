# Gemini Agent Instruction & Repository Rules

> **Auto-loaded by Gemini / Antigravity IDE Agents.**
> This file primes Google Gemini models with instant cognitive context and navigational superpowers across this codebase.

---

## 1. Project Identity & Architecture

* **Repository:** `harness2-rust` (pure-Rust clean-room implementation of Google Cloud AI Research's **Harness²** architecture).
* **Target LLM Runtime:** Optimized for compact models (e.g. Liquid AI's `LFM2.5-350M`) running locally on Ollama.
* **Workspace Crates:**
  * `crates/ecoharness` (`harness2`): Headless Rust SDK. Zero GUI dependencies.
  * `crates/ecoharness-gui` (`harness2-gui`): Native 2-pane desktop studio built with `iced` (0.14) and `lucide-icons`.
* **Deep Cognitive Reference:** See [aidocs/GEMINI_COCKPIT.md](file:///C:/RustProjects/harness2-rust/aidocs/GEMINI_COCKPIT.md) for the symbol map, line numbers, and fast tool recipes.

---

## 2. Invariant Rules for Gemini Agents

1. **Keep `ecoharness` 100% Headless:** Never add GUI, windowing, or Iced dependencies to `crates/ecoharness`.
2. **Preserve the 8-Component Structure:** The 8 canonical files (`system_prompt.md`, `guardrails.md`, `memory.md`, `tool_guidance.md`, `subagents.json`, `skills/manifest.md`, `scripts/manifest.md`, `plugins/manifest.md`) must always be maintained.
3. **Transactional JSON Rollback:** If `subagents.json` receives invalid JSON, immediately reject the patch and roll back to the pre-patch state.
4. **Isolate Probes:** In Plan Mode (`PPC`), always probe candidate diffs inside sandboxes (`ws.clone_to_sandbox`), never in the base workspace.
5. **Guardrail Enforcer Priority:** Always enforce `GuardrailEnforcer::validate_tool_call` *before* spawning any subprocess via `ScriptRunner`.
6. **Zero-Network Unit Tests:** All tests run via `cargo test --workspace` must pass completely offline with `wiremock`. The real Ollama hardware test is marked `#[ignore]`.

---

## 3. Fast Command Cheatsheet

* **Run deterministic tests:** `cargo test --workspace`
* **Check compile:** `cargo check --workspace`
* **Run CLI developer example:** `cargo run -p ecoharness --example invoice_agent`
* **Launch Desktop Studio:** `cargo run -p ecoharness-gui`
