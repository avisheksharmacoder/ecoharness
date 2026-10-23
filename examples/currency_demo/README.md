# LFM2.5-350M Self-Healing Tool Guidance Benchmark

This benchmark demonstrates **LFM2.5-350M** running on Ollama, failing on an initial single-pass turn due to an outdated tool contract, diagnosing its own schema failure, and autonomously self-improving its harness in **Plan Mode (Propose-Probe-Compose)**.

---

## The Failure Mode: Currency Strings vs Integer Cents
Small models (~350M parameters) frequently emit raw currency strings (`"$125.50"`) instead of integer cents (`12550`) and uppercase currency codes (`"USD"`). 

By mounting `flawed_tool_guidance.md` and attaching `invoice_8842.json`, an embedded **Self-Audit Gate** forces `LFM2.5-350M` to:
1. Score the unpatched Reference run at `0.25` with `GATEWAY_REJECTED`.
2. Propose a patch to `ToolGuidance`.
3. Score the patched Probe run at `0.98` with `error: null`.
4. Compose the winning patch and permanently upgrade `tool_guidance.md` on disk.

---

## Step-by-Step GUI Walkthrough (`ecoharness-gui`)

1. **Launch Studio**:
   ```bash
   cargo run -p ecoharness-gui
   ```

2. **Select & Configure Model**:
   * Click **Sync** next to **Ollama Models**.
   * Select `lfm2.5-350m:latest` (or `LiquidAI/lfm2.5-350m`).
   * In the bottom-left Detail Sub-Pane, click **Set Both** and ensure **Enforce Strict JSON Schema** is checked.

3. **Mount `flawed_tool_guidance.md`**:
   * In the Left Pane, select **Session 1** and open the **ToolGuidance** component tab.
   * In the Right Pane's bottom file bar, click **Mount to ToolGuidance**.
   * Click **Add File** and select `flawed_tool_guidance.md`.
   * Confirm the editor buffer updates to `# Tool Guidance (Legacy v1 — Incomplete)`.

4. **Attach `invoice_8842.json` into Prompt Context**:
   * In the Right Pane's bottom file bar, click **Route to Prompt**.
   * Click **Add File** and select `invoice_8842.json`.
   * Confirm the staged file counter shows `Staged files: 1`.

5. **Run Baseline (Plan Mode OFF — Watch It Fail)**:
   * Keep **Plan Mode (Recursive PPC)** **OFF**.
   * Copy the prompt from `prompt.txt`:
     ```text
     Process the attached invoice_8842.json by calling charge_invoice. Strictly obey the Self-Audit Scoring Rubric in [ToolGuidance]: if [ToolGuidance] does not explicitly define integer "amount_cents" and uppercase "currency", report the GATEWAY_REJECTED error with score 0.25. When proposing or composing harness diffs, patch ToolGuidance so charge_invoice requires integer "amount_cents" (12550) and uppercase "currency" ("USD").
     ```
   * Click **Send**.
   * Observe output: `score: 0.25`, `error: "GATEWAY_REJECTED: tool_guidance.md is missing integer amount_cents and uppercase currency rules"`.

6. **Run Self-Healing (Plan Mode ON — Watch It Adapt)**:
   * Toggle **Plan Mode (Recursive PPC)** **ON**.
   * Send the same prompt again.
   * Watch the status bar: `Plan Mode: Propose -> Probe -> Compose...`.
   * Under the hood:
     - Reference pass fails at `0.25`.
     - Invariant patch generated for `ToolGuidance`.
     - Invariant probe succeeds at `0.98`.
     - Compose adopts `ToolGuidance` patch (`0.98 > 0.25`) and updates `EXPERIENCE.md`.
   * Observe output: Assistant bubble shows `✓ Plan Mode Adopted Harness Diffs: ToolGuidance` with `score: 0.98`.
   * The Left Pane's `ToolGuidance` buffer is now permanently upgraded!
