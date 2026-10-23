# Open Model Deployment & Configuration Guide

EcoHarness is designed to run locally with **open weights models** via **Ollama**. This guide details how to configure, tune, and pair different model families and parameter sizes—from ultra-compact 350M edge models to 7B–14B coding and reasoning models.

---

## 1. Model Role Taxonomy

In EcoHarness, models are assigned roles in the cognitive loop:

| Role | Responsibility | Recommended Model Capabilities |
| :--- | :--- | :--- |
| **Executor** | Executes task trajectories, parses files, formats JSON, and triggers tools. | High tool-calling precision, deterministic JSON output, fast latency. |
| **Editor** | Critiques execution traces, diagnoses failures, proposes `(Invariant, Domain)` candidate diffs, and composes winning lessons. | Strong reasoning, analytical critique, long-context comprehension. |
| **Both** | A single model serves as both Executor and Editor. | Balanced general-purpose models (e.g. `llama3.2`, `qwen2.5:3b`, `lfm2.5-350m`). |

---

## 2. Model Categories & Recommended Settings

### Category A: Ultra-Compact Edge Models (350M – 1B Parameters)
Designed for embedded, edge, or low-power CPU environments.

* **Target Models:**
  * **Liquid AI `LFM2.5-350M`** (GGUF 4-bit `Q4_0` or `Q4_K_M`)
  * **Alibaba `Qwen2.5-0.5B`**
  * **Hugging Face `SmolLM2-360M`**
* **Characteristics:** Extremely fast inference, low RAM consumption (< 1 GB), but sensitive to prompt bloat and prone to JSON syntax imperfections.
* **Tuning Recommendations:**
  * **Compression Budget:** Set `TrajectoryCompressor` max chars to **900 – 1200**.
  * **Temperature:** Keep low (**`0.05 – 0.15`**) to maximize deterministic schema adherence.
  * **JSON Schema Enforcement:** Always enable (`enforce_json_schema: true`).
  * **Plugins:** In `plugins/manifest.md`, enable `repair_trailing_commas: true` to auto-repair accidental trailing commas in generated JSON.
  * **Experience Retrieval:** Rely on Top-$K$ keyword retrieval (`retrieve_relevant_experience`) so historical lessons don't flood the model's compact context.

#### Pulling LFM2.5-350M into Ollama:
```bash
# Pull direct GGUF from Hugging Face:
docker exec -it ollama ollama pull hf.co/LiquidAI/LFM2.5-350M-GGUF:Q4_0

# Alias to standard name:
docker exec -it ollama ollama cp hf.co/LiquidAI/LFM2.5-350M-GGUF:Q4_0 lfm2.5-350m
```

---

### Category B: Compact General-Purpose Models (1B – 3B Parameters)
The ideal sweet-spot for personal laptops and consumer desktops.

* **Target Models:**
  * **Meta `Llama-3.2-1B` / `Llama-3.2-3B`**
  * **Alibaba `Qwen2.5-1.5B` / `Qwen2.5-3B`**
  * **Microsoft `Phi-3.5-mini` (3.8B)**
* **Characteristics:** Strong instruction following, robust multi-turn conversation, and native structured tool calling.
* **Tuning Recommendations:**
  * **Compression Budget:** **1500 – 2500** characters.
  * **Temperature:** **`0.15 – 0.3`**.
  * **Multi-Turn Turns:** Set `max_turns: 3` to `5` for complex script chains.
  * **Role Assignment:** Excellent when used as `ModelRole::Both`.

#### Pulling Llama 3.2:
```bash
docker exec -it ollama ollama pull llama3.2
```

---

### Category C: Mid-Sized & Reasoning Models (7B – 14B Parameters)
Designed for workstations, servers, or GPU-accelerated local machines.

* **Target Models:**
  * **Alibaba `Qwen2.5-Coder-7B` / `14B`** (Premier code synthesis and script generation)
  * **Meta `Llama-3.1-8B`** (Comprehensive reasoning and critique)
  * **Mistral AI `Mistral-7B-Instruct-v0.3`**
  * **DeepSeek `DeepSeek-R1-Distill-Qwen-7B` / `8B`** (Chain-of-thought reasoning)
* **Characteristics:** Exceptional reasoning, deep critique capability, and zero JSON syntax errors.
* **Tuning Recommendations:**
  * **Compression Budget:** **3000 – 6000** characters.
  * **Temperature:** **`0.2 – 0.5`**.
  * **Reasoning Tag Stripping:** If using DeepSeek-R1 or reasoning models that output `<think>...</think>`, enable `strip_thinking_tags` in `plugins/manifest.md` to prevent internal thought traces from polluting final JSON output fields.

#### Pulling 7B Models:
```bash
docker exec -it ollama ollama pull qwen2.5-coder:7b
docker exec -it ollama ollama pull llama3.1:8b
```

---

## 3. High-Leverage Architecture: Asymmetric Model Pairing

One of EcoHarness's most powerful architectural features is **Asymmetric Multi-Model Pairing**:

$$\begin{aligned}
\textbf{Executor Model} &\longrightarrow \textbf{Compact \& Fast} \quad (\text{e.g., } \text{LFM2.5-350M or Llama-3.2-1B}) \\
\textbf{Editor Model} &\longrightarrow \textbf{Analytical \& Deep} \quad (\text{e.g., } \text{Qwen2.5-Coder-7B or Llama-3.1-8B})
\end{aligned}$$

### Why Asymmetric Pairing?
* **High Efficiency during Execution:** Routine tasks and tool calls run on the lightweight Executor model with sub-second latency and minimal power consumption.
* **Intelligent Optimization during Plan Mode:** When a task fails or needs self-auditing, the larger Editor model inspects the error trace, analyzes the failure mode, and writes high-quality patches into the harness files.
* **Result:** The compact model inherits the wisdom of the larger model stored permanently in its 8-file harness and `EXPERIENCE.md`, without needing to run the 7B model continuously!

### Configuring Asymmetric Pairing in Code:
```rust
use ecoharness::{OllamaConfig, OllamaDriver};

let config = OllamaConfig {
    base_url: "http://localhost:11434".to_string(),
    executor_model: "llama3.2".to_string(),      // Fast compact 3B model for tasks
    editor_model: "qwen2.5-coder:7b".to_string(), // 7B reasoning model for diff synthesis
    temperature: 0.1,
    enforce_json_schema: true,
};

let driver = OllamaDriver::new(config);
```

### Configuring Asymmetric Pairing in GUI:
1. In the Left Pane under **Ollama Models**, click `llama3.2`.
2. In the detail pane below, click **`Set Executor`**.
3. Next, click `qwen2.5-coder:7b` in the model list.
4. In the detail pane below, click **`Set Editor`**.

---

## 4. Hardware Sizing & Acceleration Matrix

| Hardware Profile | Recommended Executor | Recommended Editor | Quantization | RAM / VRAM Required |
| :--- | :--- | :--- | :--- | :--- |
| **Raspberry Pi 5 / Edge CPU** | `lfm2.5-350m` | `lfm2.5-350m` | `Q4_0` / `Q4_K_M` | 4 GB RAM |
| **Standard Laptop (CPU Only)** | `lfm2.5-350m` or `llama3.2:1b` | `llama3.2:3b` | `Q4_K_M` | 8 – 16 GB RAM |
| **MacBook (Apple Silicon M1-M4)** | `llama3.2:3b` | `qwen2.5-coder:7b` | `Q4_K_M` or `Q8_0` | 16 – 24 GB Unified Memory |
| **Workstation (NVIDIA RTX 3060/4070+)** | `llama3.2:3b` | `llama3.1:8b` or `qwen2.5-coder:14b` | `Q4_K_M` | 8 – 12 GB VRAM |
