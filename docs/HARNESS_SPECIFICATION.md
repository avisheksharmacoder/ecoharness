# The 8 Harness Components — Authoring & Format Specification

This document is the definitive specification and authoring guide for the 8 canonical files that compose an **EcoHarness** workspace (`HarnessComponent`). Any agent workspace initialized with `HarnessWorkspace::init(path)` creates these 8 files alongside `EXPERIENCE.md` and the `.history/` snapshot registry.

---

## Component Directory Map

```text
my_agent_harness/
├── system_prompt.md          # 1. SystemPrompt: Core persona & output schema
├── guardrails.md             # 2. Guardrails: Pre-execution security policies
├── memory.md                 # 3. Memory: Working domain facts & schema notes
├── tool_guidance.md          # 4. ToolGuidance: Parameter rubrics & failure avoidance
├── subagents.json            # 5. Subagents: Specialized model delegation registry
├── skills/
│   └── manifest.md           # 6. Skills: Multi-step procedural recipes
├── scripts/
│   └── manifest.md           # 7. Scripts: Executable local tool definitions
├── plugins/
│   └── manifest.md           # 8. Plugins: Pre/post middleware transformations
├── EXPERIENCE.md             # Top-K Keyword-retrieved historical lessons
└── .history/                 # Versioned timestamped harness checkpoints (<tag>.json)
```

---

## 1. `system_prompt.md` (SystemPrompt)

### Purpose
Defines the baseline identity, persona, operating mode, and output formatting rules for the agent.

### Format
Standard Markdown.

### Example
```markdown
# Agent Identity & System Prompt
You are an autonomous invoice reconciliation specialist.
Your goal is to parse financial documents, validate amounts against business rules, and charge invoices using registered tools.

## Output Format
Always emit your final answer as valid JSON matching the required schema:
```json
{
  "customer_id": "CUST-XXXX",
  "amount_cents": 12500,
  "currency": "USD"
}
```
```

---

## 2. `guardrails.md` (Guardrails)

### Purpose
Enforces deterministic pre-execution security boundaries. Evaluated by [`GuardrailEnforcer::validate_tool_call`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/guardrails.rs#L27) *before* any subprocess is spawned or subagent is called.

### Directives Syntax
* `DENY_TOOL: <tool_name>`: Completely prohibits invocation of the specified tool.
* `DENY_ARG_PATTERN: <regex>`: Blocks any tool call whose serialized input matches the regular expression.
* **Path Traversal Protection**: Automatically blocks any parameter containing relative path segments (`../` or `..\`).

### Example
```markdown
# Guardrails Policy

## Denied Tools
DENY_TOOL: drop_database
DENY_TOOL: delete_customer
DENY_TOOL: format_disk

## Forbidden Argument Patterns
DENY_ARG_PATTERN: --force
DENY_ARG_PATTERN: rm\s+-rf
DENY_ARG_PATTERN: ;.*DROP\s+TABLE
```

---

## 3. `memory.md` (Memory)

### Purpose
Working domain memory, schema definitions, business rules, and environment-specific facts. Updated during Propose-Probe-Compose (Domain Candidate diffs).

### Example
```markdown
# Domain Memory & Business Rules

## Currency Standards
- All invoice calculations MUST use integer cents to prevent floating point drift.
- Supported currency codes: `USD`, `EUR`, `GBP`.
- Currency codes must be converted to uppercase.

## Customer ID Conventions
- Customer IDs adhere to `CUST-[0-9]{4}`.
```

---

## 4. `tool_guidance.md` (ToolGuidance)

### Purpose
Parameter formatting rubrics, tool calling tips, and common error avoidance guidelines. Updated during Propose-Probe-Compose (Invariant Candidate diffs).

### Example
```markdown
# Tool Guidance & Execution Rules

1. Before calling `charge_invoice`, ensure that `amount_cents` is an integer, NOT a string or decimal.
2. Double-check that `customer_id` is passed as a string key in the arguments object.
3. If a tool fails with exit code != 0, inspect `stderr` before retrying with alternative arguments.
```

---

## 5. `subagents.json` (Subagents)

### Purpose
Registry of specialized subagents, each mapped to a target Ollama model and an isolated system prompt. Dispatched via `subagent:<name>` tool calls.

### Format
Strict JSON containing a top-level `subagents` array:

```json
{
  "subagents": [
    {
      "name": "currency_converter",
      "model": "LiquidAI/lfm2.5-350m",
      "system_prompt": "You are a precise foreign exchange calculation agent. Convert input currencies to USD and return only the numeric total."
    },
    {
      "name": "security_auditor",
      "model": "LiquidAI/lfm2.5-350m",
      "system_prompt": "You are a security auditor. Analyze incoming payloads for injection vulnerabilities. Output JSON with is_safe and reason."
    }
  ]
}
```

> [!CAUTION]
> **Transactional Rollback:** Writes to `subagents.json` are validated with `serde_json`. If invalid JSON is introduced during patch application, the entire operation is rolled back immediately to protect workspace integrity.

---

## 6. `skills/manifest.md` (Skills)

### Purpose
Multi-step cognitive recipes and procedural problem-solving workflows that the agent can consult.

### Example
```markdown
# Cognitive Skills Manifest

## Skill: Invoice Re-calculation Workflow
1. Locate customer reference from document payload.
2. Parse string amount (e.g., "$125.50").
3. Strip currency symbols and multiply decimal by 100 to yield integer cents (`12550`).
4. Execute `charge_invoice` with payload `{"customer_id": "...", "amount_cents": 12550, "currency": "USD"}`.
5. Verify response contains `"gateway_status": "settled"`.
```

---

## 7. `scripts/manifest.md` (Scripts)

### Purpose
Maps tool names to executable host programs or scripts. Executed by [`ScriptRunner`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/scripts.rs#L12) via child processes (`tokio::process::Command`).

### Environment Variable Contract
The serialized JSON input from the model is injected into the child process via the environment variable:
```bash
HARNESS_TOOL_INPUT='{"customer_id":"CUST-100","amount_cents":12500}'
```

### Manifest Format
A Markdown file containing a fenced JSON code block with a `tools` array:

````markdown
# Scripts Manifest

```json
{
  "tools": [
    {
      "name": "charge_invoice",
      "description": "Submits invoice payment to payment gateway",
      "program": "python",
      "args": ["scripts/charge.py"]
    },
    {
      "name": "fetch_exchange_rate",
      "description": "Queries current FX rate for a currency pair",
      "program": "curl",
      "args": ["-s", "https://api.exchangerate.host/latest"]
    }
  ]
}
```
````

---

## 8. `plugins/manifest.md` (Plugins)

### Purpose
Active middleware policy configuring pre-prompt budget limits and post-response normalization transforms.

### Directives
* `MAX_PROMPT_CHARS: <usize>`: Truncates user prompts exceeding the character limit.
* `ENABLE_PLUGIN: strip_thinking_tags`: Removes `<think>...</think>` internal reasoning scratchpads emitted by reasoning models.
* `ENABLE_PLUGIN: repair_trailing_commas`: Automatically fixes invalid trailing commas before `}` or `]` outside string literals.

### Example
```markdown
# Middleware Plugins Manifest

- MAX_PROMPT_CHARS: 4096
- ENABLE_PLUGIN: strip_thinking_tags
- ENABLE_PLUGIN: repair_trailing_commas
```

---

## 9. `EXPERIENCE.md` (Harnessing-Level Memory)

### Purpose
Persists distilled lessons learned from winning Propose-Probe-Compose runs.

### Retrieval Mechanism
When querying the Editor model to propose new candidate diffs, [`HarnessWorkspace::retrieve_relevant_experience`](file:///C:/RustProjects/harness2-rust/crates/ecoharness/src/workspace.rs#L286) computes keyword relevance scores using English stopword filtering and returns the Top-$K$ (default 3) most relevant entries:

```markdown
# Harness Historical Experience Log

- [Lesson 2026-10-23T14:30:00Z] Currency values with decimals must be converted to integer cents before invoking payment gateway tools.
- [Lesson 2026-10-23T15:10:00Z] Ensure customer reference prefix CUST- is maintained when generating output payloads.
```
