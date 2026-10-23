use harness2::{
    FileMounter, HarnessComponent, HarnessEngine, HarnessWorkspace, JsonFieldType,
    JsonSchemaEvaluator, MountTarget, OllamaConfig, OllamaDriver, PpcStageEvent,
};
use std::env;
use std::fs;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize or load the 8-component Harness² workspace on disk
    let root_dir = env::temp_dir().join("harness2_cli_demo");
    let probe_dir = env::temp_dir().join("harness2_cli_probes");
    let ws = HarnessWorkspace::init(&root_dir)?;

    println!("Initialized 8-component workspace at: {:?}", ws.root());

    // 2. Configure active runtime Guardrails (blocks dangerous tools & path traversal)
    ws.write_component(
        HarnessComponent::Guardrails,
        "# Guardrails\nDENY_TOOL: delete_customer\nDENY_ARG_PATTERN: --drop-table\n",
    )?;

    // 3. Mount a deterministic local tool inside the Scripts component slot
    #[cfg(unix)]
    let (prog, args) = (
        "sh",
        vec![
            "-c",
            "printf '{\"gateway_status\":\"settled\",\"raw\":%s}' \"$HARNESS_TOOL_INPUT\"",
        ],
    );
    #[cfg(windows)]
    let (prog, args) = (
        "cmd",
        vec![
            "/C",
            "echo {\"gateway_status\":\"settled\",\"raw\":%HARNESS_TOOL_INPUT%}",
        ],
    );

    let scripts_manifest = format!(
        "# Scripts Manifest\n```json\n{}\n```\n",
        serde_json::json!({
            "tools": [
                {
                    "name": "charge_invoice",
                    "description": "Charges a customer invoice in integer cents",
                    "program": prog,
                    "args": args
                }
            ]
        })
    );
    ws.write_component(HarnessComponent::Scripts, &scripts_manifest)?;

    // 4. Create an objective JsonSchemaEvaluator so the LLM cannot hallucinate its own score
    let evaluator = Arc::new(
        JsonSchemaEvaluator::new()
            .require_field("customer_id", JsonFieldType::String)
            .require_field("amount_cents", JsonFieldType::Integer)
            .require_field("currency", JsonFieldType::String),
    );

    // 5. Configure the Ollama driver (targeting LiquidAI/lfm2.5-350m by default)
    let model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "lfm2.5-350m".to_string());
    let driver = OllamaDriver::new(OllamaConfig {
        base_url: env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string()),
        executor_model: model.clone(),
        editor_model: model,
        temperature: 0.1,
        enforce_json_schema: true,
    })
    .with_max_turns(3)
    .with_script_execution(true)
    .with_evaluator(evaluator);

    // 6. Stage an external JSON file into the prompt context via FileMounter
    let sample_invoice = root_dir.join("invoice_8842.json");
    fs::write(
        &sample_invoice,
        r#"{"customer_ref": "CUST-8842", "total_display": "$125.50", "curr": "usd"}"#,
    )?;
    FileMounter::mount_file(&ws, &driver, &sample_invoice, MountTarget::PromptContext)?;

    // 7. Save a named baseline checkpoint before running the recursive loop
    ws.save_versioned_snapshot("v1_baseline")?;

    // 8. Execute the recursive Propose -> Probe -> Compose loop with live progress events
    let engine = HarnessEngine::new(driver, 1200);
    let task_prompt = "Extract customer_ref, convert $125.50 to integer amount_cents (12550), \
                       normalize currency to uppercase USD, call charge_invoice, and return \
                       final JSON with customer_id, amount_cents, and currency.";

    println!("Starting Propose -> Probe -> Compose loop...");

    let decision = engine
        .run_propose_probe_compose_with_progress(&ws, &probe_dir, task_prompt, |stage| {
            match stage {
                PpcStageEvent::ReferenceStarted => {
                    println!("  [1/5] Running Reference trajectory on base harness...");
                }
                PpcStageEvent::ReferenceCompleted { score, succeeded } => {
                    println!(
                        "  [2/5] Reference finished (score: {:.2}, succeeded: {})",
                        score, succeeded
                    );
                }
                PpcStageEvent::CandidatesProposed {
                    invariant_diff_count,
                    domain_diff_count,
                } => {
                    println!(
                        "  [3/5] Proposed {} Invariant diffs & {} Domain diffs",
                        invariant_diff_count, domain_diff_count
                    );
                }
                PpcStageEvent::ProbesCompleted {
                    invariant_score,
                    domain_score,
                } => {
                    println!(
                        "  [4/5] Concurrent Probes finished (Invariant: {:.2}, Domain: {:.2})",
                        invariant_score, domain_score
                    );
                }
                PpcStageEvent::Composed {
                    adopted_diff_count,
                    winning_score,
                } => {
                    println!(
                        "  [5/5] Compose complete! Adopted {} diffs (Winning score: {:.2})",
                        adopted_diff_count, winning_score
                    );
                }
            }
        })
        .await?;

    println!("\n=== FINAL DECISION ===");
    println!("Winning Output : {:?}", decision.winning_trajectory.final_output);
    println!("Lesson Learned : {}", decision.lesson_learned);
    println!(
        "Adopted Diffs  : {:?}",
        decision
            .adopted_diffs
            .iter()
            .map(|d| d.component)
            .collect::<Vec<_>>()
    );

    Ok(())
}
