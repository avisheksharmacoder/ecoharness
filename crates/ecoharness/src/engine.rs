use crate::trajectory::{CompressedTrajectory, Trajectory, TrajectoryCompressor};
use crate::workspace::{ExperienceEntry, HarnessComponent, HarnessWorkspace};
use crate::HarnessError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Harness² splits candidate proposals into two orthogonal axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateKind {
    /// General planning, verification, and tool-use habits.
    Invariant,
    /// Task-specific domain rules, schemas, and environment knowledge.
    Domain,
}

/// A proposed replacement for a single harness component file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDiff {
    pub component: HarnessComponent,
    pub updated_content: String,
    pub rationale: String,
}

/// A bundle of component diffs representing one candidate harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessPatch {
    pub kind: CandidateKind,
    pub diffs: Vec<ComponentDiff>,
}

/// Final verdict produced by the Composer step after inspecting probe traces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionDecision {
    pub adopted_diffs: Vec<ComponentDiff>,
    pub lesson_learned: String,
    pub winning_trajectory: Trajectory,
}

/// Live stage progress events emitted during a Plan Mode `Propose -> Probe -> Compose` run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PpcStageEvent {
    ReferenceStarted,
    ReferenceCompleted {
        score: f32,
        succeeded: bool,
    },
    CandidatesProposed {
        invariant_diff_count: usize,
        domain_diff_count: usize,
    },
    ProbesCompleted {
        invariant_score: f32,
        domain_score: f32,
    },
    Composed {
        adopted_diff_count: usize,
        winning_score: f32,
    },
}

/// Trait boundary isolating the core engine from live LLM network sockets.
#[async_trait]
pub trait ExecutionDriver: Send + Sync {
    /// Runs the task using the 8 component files currently mounted in `workspace`.
    async fn execute_task(
        &self,
        workspace: &HarnessWorkspace,
        task_prompt: &str,
    ) -> Result<Trajectory, HarnessError>;

    /// Proposes `(Invariant, Domain)` candidate patches from the compressed reference run.
    async fn propose_candidates(
        &self,
        workspace: &HarnessWorkspace,
        compressed_ref: &CompressedTrajectory,
        experience_log: &str,
    ) -> Result<(HarnessPatch, HarnessPatch), HarnessError>;

    /// Contrastively evaluates the reference, Invariant probe, and Domain probe traces to
    /// synthesize the final composed diff.
    async fn compose(
        &self,
        workspace: &HarnessWorkspace,
        ref_trace: &CompressedTrajectory,
        invariant_patch: &HarnessPatch,
        invariant_trace: &CompressedTrajectory,
        domain_patch: &HarnessPatch,
        domain_trace: &CompressedTrajectory,
    ) -> Result<CompositionDecision, HarnessError>;
}

/// The recursive Harness² orchestrator.
pub struct HarnessEngine<D: ExecutionDriver> {
    driver: D,
    compressor_max_chars: usize,
}

impl<D: ExecutionDriver> HarnessEngine<D> {
    pub fn new(driver: D, compressor_max_chars: usize) -> Self {
        Self {
            driver,
            compressor_max_chars,
        }
    }

    pub fn driver(&self) -> &D {
        &self.driver
    }

    /// Standard single-pass execution (used when Plan Mode is OFF).
    pub async fn run_single_pass(
        &self,
        workspace: &HarnessWorkspace,
        task_prompt: &str,
    ) -> Result<Trajectory, HarnessError> {
        self.driver.execute_task(workspace, task_prompt).await
    }

    /// Full Propose -> Probe (Concurrent Invariant & Domain Sandboxes) -> Compose loop.
    /// Commits adopted diffs and appends to `EXPERIENCE.md` ONLY if the winning trajectory
    /// strictly improves upon the reference trajectory's score.
    pub async fn run_propose_probe_compose(
        &self,
        base_workspace: &HarnessWorkspace,
        probe_root: &Path,
        task_prompt: &str,
    ) -> Result<CompositionDecision, HarnessError> {
        self.run_propose_probe_compose_with_progress(base_workspace, probe_root, task_prompt, |_| {})
            .await
    }

    /// Runs the full Propose -> Probe -> Compose loop while invoking `on_stage` at each milestone,
    /// using Top-K `EXPERIENCE.md` retrieval and automatic pre-commit snapshot versioning.
    pub async fn run_propose_probe_compose_with_progress<F>(
        &self,
        base_workspace: &HarnessWorkspace,
        probe_root: &Path,
        task_prompt: &str,
        mut on_stage: F,
    ) -> Result<CompositionDecision, HarnessError>
    where
        F: FnMut(PpcStageEvent) + Send,
    {
        on_stage(PpcStageEvent::ReferenceStarted);

        // 1. Run Reference Trajectory on the unmutated base harness
        let ref_traj = self.driver.execute_task(base_workspace, task_prompt).await?;
        on_stage(PpcStageEvent::ReferenceCompleted {
            score: ref_traj.score,
            succeeded: ref_traj.succeeded,
        });

        let compressed_ref =
            TrajectoryCompressor::compress(&ref_traj, self.compressor_max_chars)?;
        let experience_log = base_workspace.retrieve_relevant_experience(task_prompt, 3)?;

        // 2. Propose orthogonal Invariant & Domain candidate patches
        let (invariant_patch, domain_patch) = self
            .driver
            .propose_candidates(base_workspace, &compressed_ref, &experience_log)
            .await?;

        on_stage(PpcStageEvent::CandidatesProposed {
            invariant_diff_count: invariant_patch.diffs.len(),
            domain_diff_count: domain_patch.diffs.len(),
        });

        // 3. Clone isolated sandboxes and apply each candidate patch
        let inv_sandbox_dir = probe_root.join("probe_invariant");
        let dom_sandbox_dir = probe_root.join("probe_domain");

        let inv_ws = base_workspace.clone_to_sandbox(&inv_sandbox_dir)?;
        let dom_ws = base_workspace.clone_to_sandbox(&dom_sandbox_dir)?;

        inv_ws.apply_patch(&invariant_patch)?;
        dom_ws.apply_patch(&domain_patch)?;

        // 4. Probe both candidate harnesses concurrently
        let (inv_traj, dom_traj) = tokio::try_join!(
            self.driver.execute_task(&inv_ws, task_prompt),
            self.driver.execute_task(&dom_ws, task_prompt)
        )?;

        on_stage(PpcStageEvent::ProbesCompleted {
            invariant_score: inv_traj.score,
            domain_score: dom_traj.score,
        });

        let compressed_inv =
            TrajectoryCompressor::compress(&inv_traj, self.compressor_max_chars)?;
        let compressed_dom =
            TrajectoryCompressor::compress(&dom_traj, self.compressor_max_chars)?;

        // 5. Contrastively compose the winning patches
        let mut decision = self
            .driver
            .compose(
                base_workspace,
                &compressed_ref,
                &invariant_patch,
                &compressed_inv,
                &domain_patch,
                &compressed_dom,
            )
            .await?;

        // 6. Strict Regression Gate: Only commit to base workspace & EXPERIENCE.md
        // if the composed result strictly beats the reference baseline score.
        let best_probe_score = inv_traj.score.max(dom_traj.score);
        if decision.winning_trajectory.score <= ref_traj.score
            || best_probe_score <= ref_traj.score
            || decision.adopted_diffs.is_empty()
        {
            decision.adopted_diffs.clear();
            decision.winning_trajectory = ref_traj;
            on_stage(PpcStageEvent::Composed {
                adopted_diff_count: 0,
                winning_score: decision.winning_trajectory.score,
            });
            return Ok(decision);
        }

        // Save an automatic rollback checkpoint prior to mutating the base workspace
        let _ = base_workspace.save_versioned_snapshot("pre_ppc_auto");

        let composed_patch = HarnessPatch {
            kind: CandidateKind::Invariant,
            diffs: decision.adopted_diffs.clone(),
        };
        base_workspace.apply_patch(&composed_patch)?;

        let entry = ExperienceEntry {
            task_summary: task_prompt.to_string(),
            adopted_components: decision
                .adopted_diffs
                .iter()
                .map(|d| d.component)
                .collect(),
            lesson: decision.lesson_learned.clone(),
        };
        base_workspace.append_experience(&entry)?;

        on_stage(PpcStageEvent::Composed {
            adopted_diff_count: decision.adopted_diffs.len(),
            winning_score: decision.winning_trajectory.score,
        });

        Ok(decision)
    }
}
