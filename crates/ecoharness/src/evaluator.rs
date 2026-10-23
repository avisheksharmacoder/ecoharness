use crate::ollama::OllamaDriver;
use crate::trajectory::{Trajectory, TrajectoryStep};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Expected JSON value type for a required field in `JsonSchemaEvaluator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsonFieldType {
    String,
    Integer,
    Number,
    Boolean,
    Object,
    Array,
}

impl JsonFieldType {
    fn matches(&self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Integer => value.is_i64() || value.is_u64(),
            Self::Number => value.is_number(),
            Self::Boolean => value.is_boolean(),
            Self::Object => value.is_object(),
            Self::Array => value.is_array(),
        }
    }
}

/// Objective evaluation outcome computed after an Executor run finishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationVerdict {
    pub succeeded: bool,
    pub score: f32,
    pub feedback_error: Option<String>,
}

/// Deterministic evaluator that grades a `Trajectory` independently of the LLM's self-reported score.
pub trait TaskEvaluator: Send + Sync + std::fmt::Debug {
    fn evaluate(&self, trajectory: &Trajectory) -> EvaluationVerdict;

    /// Evaluates the trajectory and overwrites `trajectory.succeeded`, `trajectory.score`,
    /// and (if validation failed) appends/updates the validation error on the final step.
    fn apply_to_trajectory(&self, trajectory: &mut Trajectory) {
        let verdict = self.evaluate(trajectory);
        trajectory.succeeded = verdict.succeeded;
        trajectory.score = verdict.score;

        if let Some(err_msg) = verdict.feedback_error {
            if let Some(last_step) = trajectory.steps.last_mut() {
                match &mut last_step.error {
                    Some(existing) if !existing.is_empty() => {
                        existing.push_str(&format!(" | {}", err_msg));
                    }
                    _ => {
                        last_step.error = Some(err_msg);
                    }
                }
            } else {
                trajectory.steps.push(TrajectoryStep {
                    turn: 1,
                    thought: None,
                    tool_name: None,
                    tool_input: None,
                    tool_output: None,
                    error: Some(err_msg),
                });
            }
        }
    }
}

/// Verifies that `trajectory.final_output` is valid JSON containing all required keys with exact types.
#[derive(Debug, Clone, Default)]
pub struct JsonSchemaEvaluator {
    required_fields: BTreeMap<String, JsonFieldType>,
}

impl JsonSchemaEvaluator {
    pub fn new() -> Self {
        Self {
            required_fields: BTreeMap::new(),
        }
    }

    pub fn require_field(mut self, key: impl Into<String>, field_type: JsonFieldType) -> Self {
        self.required_fields.insert(key.into(), field_type);
        self
    }

    pub fn required_fields(&self) -> &BTreeMap<String, JsonFieldType> {
        &self.required_fields
    }
}

impl TaskEvaluator for JsonSchemaEvaluator {
    fn evaluate(&self, trajectory: &Trajectory) -> EvaluationVerdict {
        let raw_output = match trajectory.final_output.as_deref() {
            Some(s) if !s.trim().is_empty() => s,
            _ => {
                return EvaluationVerdict {
                    succeeded: false,
                    score: 0.0,
                    feedback_error: Some(
                        "Evaluator error: final_output is missing or empty".to_string(),
                    ),
                };
            }
        };

        let clean_json = OllamaDriver::sanitize_json_payload(raw_output);
        let parsed: Value = match serde_json::from_str(clean_json) {
            Ok(v) => v,
            Err(err) => {
                return EvaluationVerdict {
                    succeeded: false,
                    score: 0.0,
                    feedback_error: Some(format!(
                        "Evaluator error: final_output is not valid JSON ({})",
                        err
                    )),
                };
            }
        };

        let obj = match parsed.as_object() {
            Some(map) => map,
            None => {
                return EvaluationVerdict {
                    succeeded: false,
                    score: 0.0,
                    feedback_error: Some(
                        "Evaluator error: final_output must be a JSON object".to_string(),
                    ),
                };
            }
        };

        if self.required_fields.is_empty() {
            return EvaluationVerdict {
                succeeded: true,
                score: 1.0,
                feedback_error: None,
            };
        }

        let mut matched = 0usize;
        let mut errors = Vec::new();

        for (field, expected_type) in &self.required_fields {
            match obj.get(field) {
                Some(val) if expected_type.matches(val) => {
                    matched += 1;
                }
                Some(val) => {
                    errors.push(format!(
                        "field '{}' has wrong type (expected {:?}, got {})",
                        field, expected_type, val
                    ));
                }
                None => {
                    errors.push(format!(
                        "missing required field '{}' (expected {:?})",
                        field, expected_type
                    ));
                }
            }
        }

        let total = self.required_fields.len();
        let score = (matched as f32) / (total as f32);

        if errors.is_empty() {
            EvaluationVerdict {
                succeeded: true,
                score: 1.0,
                feedback_error: None,
            }
        } else {
            EvaluationVerdict {
                succeeded: false,
                score,
                feedback_error: Some(format!("Schema validation failed: {}", errors.join("; "))),
            }
        }
    }
}
