# EcoHarness Quickstart Tutorial: Step-by-Step Guide

Welcome to the **EcoHarness Starter Guide**! This walkthrough takes you from zero to running your first self-optimizing local agent in under 10 minutes.

---

## Prerequisites

1. **Rust (Cargo):** Standard Rust toolchain (`rustc --version` >= 1.80).
2. **Docker:** Docker engine running locally (`docker --version`).

---

## Step 1: Start the Local Ollama Server in Docker

Run the official Ollama container with port `11434` exposed to your host:

```bash
docker run -d \
  --name ollama \
  -p 11434:11434 \
  -v ollama:/root/.ollama \
  ollama/ollama:latest
```

*(If you have an NVIDIA GPU, add `--gpus=all` to the command above).*

---

## Step 2: Pull Your First Model

Pull a compact, high-performance model into the running container:

#### Option A: Meta Llama 3.2 (3B) — Highly Recommended for Beginners
```bash
docker exec -it ollama ollama pull llama3.2
```

#### Option B: Liquid AI LFM2.5-350M (GGUF 4-bit) — For Ultra-Low Resource Machines
```bash
docker exec -it ollama ollama pull hf.co/LiquidAI/LFM2.5-350M-GGUF:Q4_0
docker exec -it ollama ollama cp hf.co/LiquidAI/LFM2.5-350M-GGUF:Q4_0 lfm2.5-350m
```

Verify that the model is ready:
```bash
curl http://localhost:11434/api/tags
```

---

## Step 3: Launch the EcoHarness Studio GUI

From the root of the repository, run:

```bash
cargo run -p ecoharness-gui
```

### What Happens on Startup:
1. The app automatically initializes a persistent 8-file workspace directory in your operating system's temporary/data directory.
2. In the background, the real-time subscription queries Ollama on port `11434`.
3. Within 3 seconds, your pulled model (`llama3.2` or `lfm2.5-350m`) will appear in the **Ollama Models** list on the left and in the **Model** dropdown in the top bar!

---

## Step 4: Run Your First Task (Single-Pass Mode)

1. Ensure **Plan Mode** is **OFF** (the toggle switch in the top header).
2. In the composer at the bottom of the right pane, type:
   ```text
   Extract the customer name and total amount from: "Customer Alice Smith was billed $450.00 on 2026-10-24"
   ```
3. Click **`Send Prompt`**.
4. The agent executes the prompt using the 8 harness files and returns the structured JSON output with a task score.

---

## Step 5: Run Your First Plan Mode Self-Audit (PPC)

Now let's experience **Recursive PPC (Propose -> Probe -> Compose)**:

1. Toggle **Plan Mode** to **ON** in the top header.
2. Type an instruction that requires adherence to strict tool schemas or validation rules:
   ```text
   Verify customer invoice #8842 and ensure amounts are processed strictly in integer cents.
   ```
3. Watch the **Status Badge** in the top bar progress through all 5 stage milestones:
   - `Reference Started`: The baseline run executes.
   - `Candidates Proposed`: The Editor model synthesizes an **Invariant candidate** (systematic rules) and a **Domain candidate** (invoice schema knowledge).
   - `Probes Completed`: Both candidates run in isolated sandbox directories (`probe_invariant` and `probe_domain`).
   - `Composed`: The Composer compares the probe traces against the baseline. If a candidate improved accuracy, its winning diffs are adopted into the workspace, and a lesson is recorded into `EXPERIENCE.md`.
4. In the left pane, click `ToolGuidance` or `EXPERIENCE.md` to see the newly learned rules!

---

## Step 6: Add a Custom Script Tool

Let's equip your agent with a real local CLI tool!

1. In the Left Pane under **8-File Harness Inspector**, click the **`Scripts`** button.
2. Replace or extend the editor buffer with this tool definition:
   ````markdown
   # Scripts Manifest
   ```json
   {
     "tools": [
       {
         "name": "system_time",
         "description": "Returns the current UTC ISO timestamp",
         "program": "date",
         "args": ["-u", "+%Y-%m-%dT%H:%M:%SZ"]
       }
     ]
   }
   ```
   ````
3. Click **`Save`** above the editor.
4. Ensure the **"Enable Multi-Turn Scripts, Subagents & Plugins"** checkbox is checked in the left detail pane.
5. In the chat prompt, send:
   ```text
   Use the system_time tool to get the current timestamp and report it.
   ```
6. The agent invokes the tool, reads the output from stdout, and incorporates it into its response!

---

## Step 7: Enforce Security Guardrails

Let's prevent the agent from running dangerous tools or commands:

1. Click the **`Guardrails`** button in the Left Pane inspector.
2. Add a restriction:
   ```markdown
   # Guardrails Policy
   - DENY_TOOL: format_disk
   - DENY_ARG_PATTERN: rm -rf
   ```
3. Click **`Save`**.
4. Now, even if an adversarial prompt or external file attempts to instruct the model to run `rm -rf`, EcoHarness's pre-execution interceptor immediately blocks the subprocess before it ever spawns!

---

## Step 8: Programmatic SDK Quickstart

If you are embedding EcoHarness in your own Rust application instead of using the GUI, see the annotated standalone code example in:

👉 **[`docs/starter/sdk_quickstart.rs`](file:///home/avisheks/Documents/rust-projects/ecoharness/ecoharness/docs/starter/sdk_quickstart.rs)**
