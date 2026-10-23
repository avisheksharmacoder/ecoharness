//! EcoHarness Headless SDK Quickstart Example
//!
//! Demonstrates how to:
//! 1. Initialize an on-disk 8-file workspace.
//! 2. Configure the OllamaDriver for local model execution.
//! 3. Mount an external document into the prompt context.
//! 4. Execute a single-pass task and inspect the structured trajectory.
//! 5. Run a Plan Mode (Propose -> Probe -> Compose) self-auditing optimization.

use ecoharness::{
    FileMounter, HarnessComponent, HarnessEngine, HarnessWorkspace, MountTarget, OllamaConfig,
    OllamaDriver,
};
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Step 1: Initializing 8-File Workspace ===");
    let workspace_dir = Path::new("./starter_agent_workspace");

    // HarnessWorkspace::init generates all 8 canonical files + EXPERIENCE.md
    let workspace = if workspace_dir.join("EXPERIENCE.md").exists() {
        HarnessWorkspace::load(workspace_dir)?
    } else {
        HarnessWorkspace::init(workspace_dir)?
    };

    println!("Workspace active at: {:?}", workspace.root());
    println!(
        "Default System Prompt:\n{}",
        workspace.read_component(HarnessComponent::SystemPrompt)?
    );

    println!("\n=== Step 2: Configuring Ollama Driver ===");
    // Config automatically checks OLLAMA_BASE_URL, OLLAMA_HOST, and OLLAMA_MODEL env vars
    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
    let config = OllamaConfig {
        base_url: "http://localhost:11434".to_string(),
        executor_model: model.clone(),
        editor_model: model,
        temperature: 0.1,
        enforce_json_schema: true,
    };

    let driver = OllamaDriver::new(config)
        .with_max_turns(3)
        .with_script_execution(true)
        .with_subagents(true)
        .with_plugins(true);

    println!("\n=== Step 3: Staging External Context File ===");
    // Create a temporary data file to demonstrate FileMounter
    let sample_invoice = workspace_dir.join("sample_invoice.json");
    fs::write(
        &sample_invoice,
        r#"{"invoice_id": 9921, "vendor": "Acme Tools", "total_usd": 128.50}"#,
    )?;

    // Mount into prompt context (makes it visible to next prompt without altering harness files)
    FileMounter::mount_file(
        &workspace,
        &driver,
        &sample_invoice,
        MountTarget::PromptContext,
    )?;
    println!("Mounted {:?} into LLM prompt context", sample_invoice);

    println!("\n=== Step 4: Single-Pass Task Execution ===");
    let engine = HarnessEngine::new(driver.clone(), 1200);

    let prompt = "Extract the vendor name and convert total_usd into integer cents.";
    println!("Running prompt: '{}'...", prompt);

    let trajectory = engine.run_single_pass(&workspace, prompt).await?;
    println!("Task Succeeded: {}", trajectory.succeeded);
    println!("Deterministic Score: {:.2}", trajectory.score);
    println!(
        "Final Output:\n{}",
        trajectory.final_output.unwrap_or_default()
    );

    println!("\n=== Step 5: Recursive Plan Mode PPC (Propose -> Probe -> Compose) ===");
    let probe_root = Path::new("./starter_sandboxes");
    let engine_ppc = HarnessEngine::new(driver, 1200).with_cleanup_sandboxes(true);

    let ppc_prompt = "Audit payment rules: ensure gateway charges use integer cents and validate required fields.";
    println!("Running Plan Mode PPC: '{}'...", ppc_prompt);

    let decision = engine_ppc
        .run_propose_probe_compose_with_progress(
            &workspace,
            probe_root,
            ppc_prompt,
            |stage_event| {
                println!("  [PPC Stage Event]: {:?}", stage_event);
            },
        )
        .await?;

    println!("\n--- Composition Decision ---");
    println!("Adopted Diffs Count: {}", decision.adopted_diffs.len());
    for diff in &decision.adopted_diffs {
        println!("  Adopted diff for: {:?}", diff.component);
        println!("  Rationale: {}", diff.rationale);
    }
    println!("Lesson Learned: {}", decision.lesson_learned);
    println!("Winning Score: {:.2}", decision.winning_trajectory.score);

    // Clean up sample data file
    let _ = fs::remove_file(sample_invoice);

    println!("\n=== Quickstart Finished Successfully ===");
    Ok(())
}
