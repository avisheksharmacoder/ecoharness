use crate::HarnessError;
use serde::{Deserialize, Serialize};

/// A single turn in an agent's execution trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryStep {
    pub turn: usize,
    pub thought: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_output: Option<String>,
    pub error: Option<String>,
}

/// Complete record of an Executor run against a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    pub task_prompt: String,
    pub steps: Vec<TrajectoryStep>,
    pub final_output: Option<String>,
    pub succeeded: bool,
    pub score: f32,
}

/// A budget-constrained trajectory summary tailored for compact models like `LFM2.5-350M`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressedTrajectory {
    pub summary: String,
    pub char_len: usize,
    pub estimated_tokens: usize,
}

pub struct TrajectoryCompressor;

impl TrajectoryCompressor {
    /// Compresses a raw trajectory into `<= max_chars` while strictly preserving:
    /// 1. The `task_prompt`
    /// 2. Any step containing `error.is_some()`
    /// 3. The final outcome (`succeeded`, `score`, and `final_output`)
    pub fn compress(
        trajectory: &Trajectory,
        max_chars: usize,
    ) -> Result<CompressedTrajectory, HarnessError> {
        let header = format!("TASK: {}\n", trajectory.task_prompt);
        let footer = format!(
            "OUTCOME: succeeded={} score={:.2} final={}\n",
            trajectory.succeeded,
            trajectory.score,
            trajectory.final_output.as_deref().unwrap_or("None")
        );

        let mut error_lines = String::new();
        for step in trajectory.steps.iter().filter(|s| s.error.is_some()) {
            let line = format!(
                "ERR[turn={} tool={}]: {}\n",
                step.turn,
                step.tool_name.as_deref().unwrap_or("none"),
                step.error.as_deref().unwrap_or("")
            );
            error_lines.push_str(&line);
        }

        let mandatory_len =
            header.chars().count() + error_lines.chars().count() + footer.chars().count();

        if mandatory_len > max_chars {
            return Err(HarnessError::CompressionBudgetTooSmall { budget: max_chars });
        }

        let mut remaining_budget = max_chars - mandatory_len;
        let mut normal_lines = String::new();

        for step in trajectory.steps.iter().filter(|s| s.error.is_none()) {
            let compact_step = format!(
                "STEP[turn={} tool={} input={}]: ok\n",
                step.turn,
                step.tool_name.as_deref().unwrap_or("none"),
                step.tool_input.as_deref().unwrap_or("{}")
            );
            let step_chars = compact_step.chars().count();
            if step_chars <= remaining_budget {
                normal_lines.push_str(&compact_step);
                remaining_budget -= step_chars;
            } else {
                break;
            }
        }

        let summary = format!("{}{}{}{}", header, error_lines, normal_lines, footer);
        let char_len = summary.chars().count();
        let estimated_tokens = char_len.div_ceil(4);

        Ok(CompressedTrajectory {
            summary,
            char_len,
            estimated_tokens,
        })
    }
}
