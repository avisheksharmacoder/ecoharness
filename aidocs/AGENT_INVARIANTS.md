# Agent Invariants & Architectural Rules

AI Agents editing this codebase **MUST NOT violate** the following architectural invariants. Any edit breaking these rules will cause tests to fail and break system guarantees.

---

### Invariant 1: Headless SDK Isolation (Zero GUI in `ecoharness`)
* `crates/ecoharness` is a headless SDK designed for CLI, server, and embedded environments.
* **Never add `iced`, `rfd`, or windowing dependencies** to `crates/ecoharness/Cargo.toml`.
* All GUI logic belongs strictly inside `crates/ecoharness-gui`.

---

### Invariant 2: The Canonical 8-Component Structure
* The enum `HarnessComponent` contains exactly 8 variants:
  1. `SystemPrompt` (`system_prompt.md`)
  2. `Guardrails` (`guardrails.md`)
  3. `Memory` (`memory.md`)
  4. `ToolGuidance` (`tool_guidance.md`)
  5. `Subagents` (`subagents.json`)
  6. `Skills` (`skills/manifest.md`)
  7. `Scripts` (`scripts/manifest.md`)
  8. `Plugins` (`plugins/manifest.md`)
* `HarnessWorkspace::init(dir)` **must always** generate all 8 files with non-empty default content, plus `EXPERIENCE.md`.
* `HarnessWorkspace::load(dir)` **must validate** that all 8 files exist and that `subagents.json` contains valid JSON.

---

### Invariant 3: Transactional Rollback for `subagents.json`
* `subagents.json` is strictly validated.
* If any patch or file write attempts to set invalid JSON into `Subagents`, the operation **must fail immediately** and roll back any partial mutations atomically.

---

### Invariant 4: Sandbox Mutation Isolation
* Probing candidates in Plan Mode must use `HarnessWorkspace::clone_to_sandbox(sandbox_dir)`.
* Probes execute in separate directories (`probes/invariant`, `probes/domain`).
* Under no circumstances may a probe modify the base workspace directly. The base workspace is only mutated after the Compose step adopts winning diffs.

---

### Invariant 5: Regression Rejection in Compose
* If both candidate probes score lower than the reference pass ($S_{\text{inv}} < S_{\text{ref}}$ and $S_{\text{dom}} < S_{\text{ref}}$), the Compose step **must discard all diffs**.
* An agent must never adopt a patch that lowers task performance.

---

### Invariant 6: Pre-Execution Guardrails Check
* Tools must never be executed via `ScriptRunner` until `GuardrailEnforcer` has validated:
  1. `DENY_TOOL: <name>`
  2. `DENY_ARG_PATTERN: <pattern>`
  3. Path traversal attempts (e.g. `../` or `..\`)
* If a guardrail triggers, the tool call must be aborted immediately without spawning a subprocess, and an error message must be recorded into the trajectory.

---

### Invariant 7: Compact Model Context Preservation
* Compact models like `LFM2.5-350M` fail when context overflows.
* Always enforce budget compression via `TrajectoryCompressor`.
* Always use Top-$K$ keyword retrieval (`retrieve_relevant_experience`) instead of injecting the entire `EXPERIENCE.md` history into prompts.

---

### Invariant 8: Offline Test Determinism
* `cargo test --workspace` must pass with zero network calls and zero external services.
* All HTTP interactions in unit tests must be mocked via `wiremock`.
* The real hardware test requiring Ollama is marked with `#[ignore]` and only run explicitly via `cargo test -p ecoharness -- --ignored`.
