//! Matched runtime/tool comparisons. Fixture completion is not a quality score.
use crate::{
    controller::Controller,
    research::*,
    runtime::{self, AgentRuntime, RuntimeError},
    workspace::{Workspace, WorkspaceStore},
};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::atomic::{AtomicU32, Ordering},
    time::Instant,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub struct Case {
    pub id: String,
    pub request: ResearchRequest,
    pub assess: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Measurement {
    pub case: String,
    pub outcome: String,
    pub arm: String,
    pub research_id: ResearchId,
    pub status: Status,
    pub elapsed_ms: u128,
    pub runtime_calls: u32,
    pub usage: Usage,
    pub monetary_cost: Option<f64>,
    pub human_interventions: u32,
    pub quality_score: Option<f64>,
    pub assess: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Evaluation {
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    pub mode: String,
    pub model: String,
    pub source_tools: Vec<String>,
    pub note: String,
    pub measurements: Vec<Measurement>,
}
struct Observed<'a, R> {
    inner: &'a R,
    calls: AtomicU32,
    usage: Mutex<Usage>,
}
impl<'a, R> Observed<'a, R> {
    fn new(inner: &'a R) -> Self {
        Self {
            inner,
            calls: AtomicU32::new(0),
            usage: Mutex::new(Usage {
                tool_calls: Some(0),
                input_tokens: Some(0),
                output_tokens: Some(0),
            }),
        }
    }
}
impl<R: AgentRuntime> AgentRuntime for Observed<'_, R> {
    async fn execute(
        &self,
        assignment: Assignment,
        cancel: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let result = self.inner.execute(assignment, cancel).await;
        let mut total = self.usage.lock().await;
        if let Ok(result) = &result {
            total.tool_calls = total
                .tool_calls
                .zip(result.usage.tool_calls)
                .map(|(a, b)| a.saturating_add(b));
            total.input_tokens = total
                .input_tokens
                .zip(result.usage.input_tokens)
                .map(|(a, b)| a.saturating_add(b));
            total.output_tokens = total
                .output_tokens
                .zip(result.usage.output_tokens)
                .map(|(a, b)| a.saturating_add(b));
        } else {
            *total = Usage::default();
        }
        result
    }
}

pub async fn compare<R: AgentRuntime>(
    runtime: &R,
    cases: Vec<Case>,
    mode: &str,
    root: &Path,
    cancel: CancellationToken,
) -> Result<Evaluation, Box<dyn std::error::Error>> {
    let store = WorkspaceStore::new(root)?;
    let mut measurements = Vec::new();
    for (index, case) in cases.into_iter().enumerate() {
        case.request.validate()?;
        if !case.request.attachments.is_empty() {
            return Err(
                "evaluation cases use public sources rather than private attachments".into(),
            );
        }
        // Alternate the order to reduce systematic first-run/cache bias.
        let policy_arm = match case.request.policy {
            ResearchPolicy::Staged => "structured",
            ResearchPolicy::EvidenceAccess => "evidence_access",
            ResearchPolicy::Adaptive => "adaptive",
        };
        let arms = if index % 2 == 0 {
            [policy_arm, "single_session"]
        } else {
            ["single_session", policy_arm]
        };
        for arm in arms {
            let observed = Observed::new(runtime);
            let mut request = case.request.clone();
            if arm == "single_session" {
                request.policy = ResearchPolicy::Staged;
            }
            let workspace = Workspace::new("evaluation".into(), request)?;
            let mut controller = Controller::new(workspace, runtime::unix_seconds());
            let started = Instant::now();
            if cancel.is_cancelled() {
                controller.workspace.status = Status::Cancelled;
            } else if arm != "single_session" {
                runtime::run(&observed, &mut controller, cancel.clone()).await;
            } else {
                let mut assignment = controller
                    .assignment(runtime::unix_seconds())
                    .expect("validated allowance");
                assignment.kind = AssignmentKind::CompleteResearch;
                assignment.focus="Investigate the whole question in this one agent session, using the available sources and full supplied limits. Produce the final cited answer in draft, include the evidence, and finish. Do not rely on later planning, writing, or review sessions.".into();
                match observed.execute(assignment, cancel.clone()).await {
                    Ok(result) => {
                        let complete = result
                            .draft
                            .as_ref()
                            .is_some_and(|draft| !draft.trim().is_empty());
                        match controller.workspace.apply(result) {
                            Err(error) => {
                                controller.workspace.status = Status::Failed {
                                    reason: error.to_string(),
                                }
                            }
                            Ok(NextAction::AskUser { question }) => {
                                controller.workspace.status = Status::InputRequired { question };
                            }
                            Ok(NextAction::Investigate { .. } | NextAction::Synthesize) => {
                                controller.workspace.status = Status::Exhausted {
                                    reason: "agent requested further work".into(),
                                };
                            }
                            Ok(NextAction::Finish) if complete => {
                                controller.workspace.status = Status::Completed
                            }
                            Ok(NextAction::Finish) => {
                                controller.workspace.status = Status::Failed {
                                    reason: "agent returned no final answer".into(),
                                }
                            }
                        }
                    }
                    Err(RuntimeError::Cancelled) => controller.workspace.status = Status::Cancelled,
                    Err(RuntimeError::TimedOut) => {
                        controller.workspace.status = Status::Exhausted {
                            reason: "wall-clock limit".into(),
                        }
                    }
                    Err(error) => {
                        controller.workspace.status = Status::Failed {
                            reason: error.to_string(),
                        }
                    }
                }
            }
            store.save(&controller.workspace)?;
            let id = controller.workspace.id;
            tokio::fs::write(root.join(format!("{id}.md")), controller.workspace.report()).await?;
            let outcome = if observed.calls.load(Ordering::SeqCst) == 0 {
                "skipped"
            } else {
                match controller.workspace.status {
                    Status::Completed => "completed",
                    Status::Failed { .. } => "failed",
                    Status::InputRequired { .. } => "needs_input",
                    _ => "incomplete",
                }
            };
            measurements.push(Measurement {
                outcome: outcome.into(),
                case: case.id.clone(),
                arm: arm.into(),
                research_id: id,
                status: controller.workspace.status,
                elapsed_ms: started.elapsed().as_millis(),
                runtime_calls: observed.calls.load(Ordering::SeqCst),
                usage: observed.usage.into_inner(),
                monetary_cost: None,
                human_interventions: 0,
                quality_score: None,
                assess: case.assess.clone(),
            });
        }
    }
    let result=Evaluation {reasoning_effort:std::env::var("DEEPRESEARCH_REASONING_EFFORT").ok(),mode:mode.into(),model:std::env::var("DEEPRESEARCH_MODEL").unwrap_or_else(|_|"runtime default (record the resolved model for a publishable comparison)".into()),source_tools:std::env::var("DEEPRESEARCH_SOURCE_TOOLS").unwrap_or_default().split(',').filter(|s|!s.is_empty()).map(str::to_owned).collect(),note:"Quality and monetary cost are unscored. Fixture outputs establish only execution. Compare the saved answers blind using the case rubric; failures, input-required runs, and cancellation are not successful answers. Both arms use the same runtime, source tools and per-case wall/tool limits; structured work additionally has its assignment cap. Model context and provider-side caching may differ.".into(),measurements};
    tokio::fs::write(
        root.join("evaluation.json"),
        serde_json::to_vec_pretty(&result)?,
    )
    .await?;
    Ok(result)
}
