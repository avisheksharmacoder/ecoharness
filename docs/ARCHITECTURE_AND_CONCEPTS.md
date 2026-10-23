# EcoHarness Architecture & Theoretical Foundations

`EcoHarness` (`harness2-rust`) is a pure-Rust, zero-network-overhead clean-room implementation of Google Cloud AI Research's **Harness²** architecture. It is specifically engineered to empower local, open-source language models (from ultra-compact 350M edge models to 7B–14B coding and reasoning models) with self-auditing, recursive improvement, and tool execution without fine-tuning weights.

---

## 1. The Core Philosophy: Environment as 8 Files

Traditional agent engineering attempts to alter model capabilities through fine-tuning weights or injecting monolithic, brittle prompt strings. In contrast, the **Harness²** philosophy treats an agent's behavior as an **externally mounted, modular environment** composed of exactly **8 canonical, disk-mounted files**:

```text
harness_workspace/
├── system_prompt.md        # Slot 1: Core persona, operational mode & role directives
├── guardrails.md           # Slot 2: Non-negotiable security boundaries & denied tool rules
├── memory.md               # Slot 3: Working scratchpad, cross-session facts & user state
├── tool_guidance.md        # Slot 4: Parameter rules, API quirks, schemas & edge-case tips
├── subagents.json          # Slot 5: Specialized subagent models & isolated role configurations
├── skills/
│   └── manifest.md         # Slot 6: Reusable high-level analytical procedures & recipes
├── scripts/
│   └── manifest.md         # Slot 7: Local executable CLI subprocess tools & binaries
├── plugins/
│   └── manifest.md         # Slot 8: Token budget limits, syntax auto-repair & filter middleware
└── EXPERIENCE.md           # Persistent episodic lesson log (Harnessing-Level Recursion)
```

### Why Modular Files?
1. **Isolated Perturbation:** During optimization, the system can propose changes to a single component (e.g., updating only `tool_guidance.md` or adding a safety rule to `guardrails.md`) without destroying other agent capabilities.
2. **Deterministic Versioning:** Every workspace is backed by on-disk files. Changes can be tracked via git, rolled back atomically using `.history/<tag>.json` snapshots, and inspected by humans.
3. **No Weight Fine-Tuning:** Compact models (e.g. 350M parameters) can achieve high task reliability simply by optimizing their external harness guidance.

---

## 2. Two-Level Recursive Improvement

EcoHarness operates on two distinct recursive loops: **Task-Level Recursion** and **Harnessing-Level Recursion**.

```text
               ┌────────────────────────────────────────────────────────┐
               │                   HarnessEngine (PPC)                  │
               └──────────┬─────────────────────────────┬───────────────┘
                          │                             │
               [Reference Pass]                         │
                          │                             ▼
                          ▼                   [OllamaDriver (Models)]
                  Score: S_ref                          │
                          │             ┌───────────────┴───────────────┐
                          ▼             ▼                               ▼
                 [Propose] ──► Invariant Candidate               Domain Candidate
                                        │                               │
                                        ▼                               ▼
                               [Sandbox Probing]               [Sandbox Probing]
                                        │                               │
                                        └───────────────┬───────────────┘
                                                        │
                                                        ▼
                                                    [Compose]
                                         S_win > S_ref ? Commit : Discard
                                                        │
                                                        ▼
                                               [EXPERIENCE.md]
                                            (Top-K Keyword Ranking)
```

### 1. Task-Level Recursion: Propose-Probe-Compose (PPC)
When **Plan Mode** is enabled, instead of executing a task in a single fragile pass, `HarnessEngine` executes the PPC pipeline:

1. **Reference Pass:** The model executes the task using the current harness. The trajectory is compressed, and the task score ($S_{\text{ref}}$) is graded deterministically by a [`TaskEvaluator`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/evaluator.rs).
2. **Propose:** The Editor model inspects the reference trace, error logs, and retrieved lessons. It proposes two orthogonal candidate patch sets:
   * **Invariant Candidate:** General procedural improvements, verification strategies, and tool-use habits.
   * **Domain Candidate:** Task-specific rules, exact parameter schemas, and domain data.
3. **Isolated Sandboxed Probing:** The workspace is cloned into two isolated sandbox directories (`probe_invariant` and `probe_domain`). Neither candidate touches the base workspace. Both candidates re-run the task concurrently or sequentially, producing scores $S_{\text{inv}}$ and $S_{\text{dom}}$.
4. **Compose:** The Composer model compares the reference trace and both candidate probe traces:
   * **Regression Rejection:** If both candidate probes score lower than the reference pass ($S_{\text{inv}} < S_{\text{ref}}$ and $S_{\text{dom}} < S_{\text{ref}}$), **all diffs are discarded**. The agent will never adopt a patch that degrades performance.
   * **Selective Adoption:** If a candidate improves or fixes an error, its winning diffs are atomically committed to the base workspace.

### 2. Harnessing-Level Recursion: Persistent Experience
When a patch is adopted, a distilled lesson is recorded into `EXPERIENCE.md`.
* Compact models have limited context windows (e.g. 2K–8K tokens) and fail if entire histories are injected.
* EcoHarness implements **Top-$K$ keyword ranking retrieval** with stopword filtering (`retrieve_relevant_experience`). Only the most relevant past lessons for the current task prompt are injected into the dynamic context.

---

## 3. Subsystem Architecture

### 1. Workspace Subsystem ([`workspace.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/workspace.rs))
* **`HarnessComponent` Enum:** Defines the 8 canonical slots with relative paths and default templates.
* **`HarnessWorkspace::init(dir)`:** Generates all 8 files with non-empty templates and `EXPERIENCE.md`.
* **`HarnessWorkspace::load(dir)`:** Validates presence of all files and verifies JSON validity in `subagents.json`.
* **Atomic Rollback:** If invalid JSON is patched into `subagents.json`, the patch operation immediately rolls back any partial writes.
* **Versioned Checkpoints:** Stores full JSON snapshots in `.history/<tag>.json` with single-call rollback.

### 2. Guardrail Enforcement Subsystem ([`guardrails.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/guardrails.rs))
Security is enforced before any subprocess or tool execution:
$$\text{Tool Call Request} \longrightarrow \text{GuardrailEnforcer} \longrightarrow \begin{cases} \text{DENIED} \implies \text{Record Error into Trajectory Step} \\ \text{ALLOWED} \implies \text{Spawn Script Subprocess} \end{cases}$$

Supported directives in `guardrails.md`:
* `DENY_TOOL: <tool_name>`: Explicitly forbids tool execution.
* `DENY_ARG_PATTERN: <pattern>`: Rejects tool arguments containing forbidden strings (e.g., `rm -rf`, `DROP TABLE`).
* **Path Traversal Protection:** By default, attempts containing `../` or `..\` are unconditionally blocked.

### 3. Tool Execution Subsystem ([`scripts.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/scripts.rs))
* Tools are declared in `scripts/manifest.md` as JSON specifications (inside markdown code blocks or raw JSON).
* Each tool specifies a `program` (e.g. `python3`, `bash`, `node`, `curl`) and static `args`.
* Input arguments from the model are passed securely via the `HARNESS_TOOL_INPUT` environment variable to prevent shell injection.
* Captures `stdout`, `stderr`, and exit codes into typed [`ScriptExecutionOutput`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/scripts.rs#L25-L30).

### 4. Subagent Dispatcher ([`subagents.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/subagents.rs))
* Configured via `subagents.json`.
* When the main agent calls `subagent:<name>`, the dispatcher resolves the subagent's dedicated model and system prompt, allowing modular specialization (e.g. delegating math to a math model or coding to a coding model).

### 5. Middleware Plugins ([`plugins.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/plugins.rs))
Configured via `plugins/manifest.md`:
* `ENABLE_PLUGIN: repair_trailing_commas`: Automatically repairs trailing commas in LLM JSON outputs before parsing.
* `ENABLE_PLUGIN: strip_thinking_tags`: Strips internal reasoning tags (e.g., `<think>...</think>` from DeepSeek-R1 or Qwen models).
* `MAX_PROMPT_CHARS: <usize>`: Hard token budget truncation for small model contexts.

### 6. Objective Evaluator ([`evaluator.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/evaluator.rs))
* Compact and mid-sized LLMs frequently hallucinate their own task success.
* [`JsonSchemaEvaluator`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/evaluator.rs#L66) deterministically validates output JSON against required keys and data types, computing ground-truth scores ($0.0$ to $1.0$) and injecting diagnostic validation errors.

### 7. File Mounter ([`mounter.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/mounter.rs))
Routes external files:
* **`MountTarget::PromptContext`:** Injects external data (e.g., invoices, logs, schemas) into the active LLM context.
* **`MountTarget::HarnessSlot(component)`:** Directly overwrites one of the 8 canonical harness files.

### 8. Driver & Docker Integration ([`ollama.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/crates/ecoharness/src/ollama.rs))
* Communicates with local Ollama daemons via HTTP (`/api/tags`, `/api/chat`).
* Auto-normalizes Docker URLs (`0.0.0.0`, missing schemes, custom networks).
* Strips markdown fences (````json ... ````) and cleans malformed envelopes.
* Supports role assignment: **Executor**, **Editor**, or **Both**.
