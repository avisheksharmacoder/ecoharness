# AI Agent Guide (`aidocs/`)

Welcome, AI Agent! This directory contains concise, high-density architectural context, invariant rules, and API maps designed specifically for automated agents and LLM pair-programmers working on `harness2-rust`.

Before reading or modifying source code, review these documents:

---

## Documentation Index

| File | Purpose | When to Read |
|---|---|---|
| [GEMINI_COCKPIT.md](file:///C:/RustProjects/harness2-rust/aidocs/GEMINI_COCKPIT.md) | **Gemini & Antigravity Acceleration Matrix:** symbol line map, WireMock recipes, fast tool playbook. | **Recommended starting point for Gemini / Antigravity agents.** |
| [SYSTEM_OVERVIEW.md](file:///C:/RustProjects/harness2-rust/aidocs/SYSTEM_OVERVIEW.md) | High-level system architecture, crate decoupling, and core loops. | When starting a new task or understanding the system as a whole. |
| [AGENT_INVARIANTS.md](file:///C:/RustProjects/harness2-rust/aidocs/AGENT_INVARIANTS.md) | Non-negotiable architectural invariants and safety rules. | **Mandatory** before making any edits to ensure no regressions. |
| [DATA_STRUCTURES_AND_APIS.md](file:///C:/RustProjects/harness2-rust/aidocs/DATA_STRUCTURES_AND_APIS.md) | Core types, structs, enums, traits, and error types. | When writing new code, calling SDK methods, or wiring UI messages. |
| [WORKFLOWS_AND_TESTING.md](file:///C:/RustProjects/harness2-rust/aidocs/WORKFLOWS_AND_TESTING.md) | Cargo commands, test execution, mocking practices, and debugging. | When validating code, running tests, or building examples. |

---

## Quick Mental Model

`harness2-rust` is a pure-Rust implementation of Google Cloud AI Research's **Harness²** architecture, tailored for local compact models (like Liquid AI's `LFM2.5-350M` on Ollama):

1. **Environment as 8 Files:** Instead of fine-tuning weights, the agent's behavior is directed by 8 modular, disk-mounted files (`SystemPrompt`, `Guardrails`, `Memory`, `ToolGuidance`, `Subagents`, `Skills`, `Scripts`, `Plugins`).
2. **Task-Level Recursion (`PPC`):** Propose orthogonal Invariant/Domain patches $\rightarrow$ Probe them concurrently in isolated sandboxes $\rightarrow$ Compose winning diffs.
3. **Harnessing-Level Recursion (`EXPERIENCE.md`):** Winning lessons are persisted to disk and retrieved via Top-$K$ keyword ranking so compact models never overflow context.
4. **Two Crates:**
   * `crates/ecoharness` (`harness2`): Completely headless SDK. Zero GUI dependencies.
   * `crates/ecoharness-gui` (`harness2-gui`): Native 2-pane desktop studio built with `iced` (0.14) and `lucide-icons`.
