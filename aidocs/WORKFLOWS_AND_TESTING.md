# Developer Workflows & Testing Guide

Essential commands, testing practices, and patterns for modifying `harness2-rust`.

---

## 1. Quick CLI Commands

### Run Full Test Suite
Runs all 30 deterministic unit and integration tests (0 network sockets required):
```bash
cargo test --workspace
```

### Run SDK Unit Tests Only
```bash
cargo test -p ecoharness
```

### Run Headless GUI Unit Tests Only
```bash
cargo test -p ecoharness-gui
```

### Run the Live Ollama Hardware Smoke Gate
Requires an active Ollama server (`ollama serve`) with `LiquidAI/lfm2.5-350m` pulled:
```bash
OLLAMA_MODEL="LiquidAI/lfm2.5-350m" cargo test -p ecoharness -- --ignored
```

### Run the Standalone CLI Example
```bash
cargo run -p ecoharness --example invoice_agent
```

### Launch the 2-Pane Desktop Studio
```bash
cargo run -p ecoharness-gui
```

---

## 2. Testing Guidelines

### A. Zero-Network Test Invariant
* Never make actual HTTP requests to Ollama inside standard unit tests.
* Use `wiremock::MockServer` to simulate Ollama responses:
```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

let server = MockServer::start().await;
Mock::given(method("POST"))
    .and(path("/api/chat"))
    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "message": { "role": "assistant", "content": "..." }
    })))
    .mount(&server)
    .await;
```

### B. Headless GUI Testing
* The GUI crate (`ecoharness-gui`) uses the pure Elm Architecture pattern.
* You do **not** need a display server or window manager to test GUI logic.
* Instantiate `AppState::new(...)` and call `AppState::update(&mut state, Message::...)` directly to test state transitions:
```rust
let mut state = AppState::new(base_dir, probe_dir, OllamaConfig::default()).unwrap();
let _ = state.update(Message::SelectHarnessComponent(HarnessComponent::Guardrails));
assert!(matches!(state.detail_view, DetailPaneView::HarnessInspector { selected_component: HarnessComponent::Guardrails, .. }));
```

---

## 3. How to Extend the System

### Adding a New GUI Message
1. Add variant to `pub enum Message` in `crates/ecoharness-gui/src/state.rs`.
2. Add match branch to `AppState::update` in `state.rs`.
3. Wire the trigger in `crates/ecoharness-gui/src/view.rs` (e.g. `button("...").on_press(Message::MyNewMessage)`).
4. Add a unit test verifying the state transition in `crates/ecoharness-gui/src/tests.rs`.

### Adding a New Guardrail Rule
1. Add rule parser in `crates/ecoharness/src/guardrails.rs`.
2. Validate against tool call args in `GuardrailEnforcer::validate_tool_call`.
3. Add a test in `crates/ecoharness/src/tests.rs` to verify that violations trigger before subprocess execution.

---

## 4. Common Pitfalls to Avoid
1. **Never mutate base workspace during probing:** Always probe inside `ws.clone_to_sandbox(&probe_dir)`.
2. **Never allow `subagents.json` to be invalid JSON:** Every write to `Subagents` must be validated with `serde_json::from_str`.
3. **Never bypass `TrajectoryCompressor`:** Compact models (350M parameters) will fail or hallucinate if the chat history exceeds their prompt budget.
4. **Always update `Cargo.lock` cleanly:** Run `cargo check --workspace` after changing any dependency.
