//! A runtime executes every model assignment, including writing and review.
use crate::{controller::Controller, research::*};
use std::{
    future::Future,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum RuntimeError {
    #[error(
        "worker interrupted by service shutdown; preserved material is available for an explicit revision"
    )]
    Interrupted,
    #[error("assignment cancelled after worker termination")]
    Cancelled,
    #[error("assignment exceeded its time allowance")]
    TimedOut,
    #[error("{0}")]
    Failed(String),
}

/// Returning Cancelled means execution has stopped, not merely that cancellation
/// was requested. Implementations must clean up started external processes.
pub trait AgentRuntime: Send + Sync {
    fn execute(
        &self,
        assignment: Assignment,
        cancel: CancellationToken,
    ) -> impl Future<Output = Result<AssignmentResult, RuntimeError>> + Send;
}

pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Standalone driver. Restate uses the same controller with durable assignment calls.
pub async fn run<R: AgentRuntime>(
    runtime: &R,
    controller: &mut Controller,
    cancel: CancellationToken,
) {
    loop {
        if cancel.is_cancelled() && !controller.workspace.status.terminal() {
            controller.workspace.status = Status::Cancelled;
            return;
        }
        let Some(assignment) = controller.assignment(unix_seconds()) else {
            break;
        };
        match runtime.execute(assignment, cancel.child_token()).await {
            Ok(result) if !cancel.is_cancelled() => {
                if let Err(error) = controller.complete(result) {
                    controller.workspace.status = Status::Failed {
                        reason: error.to_string(),
                    };
                }
            }
            Ok(_) | Err(RuntimeError::Cancelled) => controller.workspace.status = Status::Cancelled,
            Err(RuntimeError::TimedOut) => {
                controller.workspace.status = Status::Exhausted {
                    reason: "assignment time allowance".into(),
                }
            }
            Err(error) => {
                controller.workspace.status = Status::Failed {
                    reason: error.to_string(),
                }
            }
        }
    }
}

/// Scripted returns test orchestration without claiming to simulate model quality.
#[derive(Clone)]
pub struct FixtureRuntime {
    pub sources: Vec<Source>,
}

impl Default for FixtureRuntime {
    fn default() -> Self {
        Self { sources: vec![
            Source { id: "S1".into(), url: "https://example.org/fixtures/option-a".into(), title: "Fixture option A".into(), excerpt: "Option A keeps completed workflow steps across a restart but external processes need reconciliation.".into(), needs_refresh: false },
            Source { id: "S2".into(), url: "https://example.org/fixtures/option-b".into(), title: "Fixture option B".into(), excerpt: "Option B has a simpler execution loop but reruns unfinished research after a restart.".into(), needs_refresh: false },
        ] }
    }
}

impl AgentRuntime for FixtureRuntime {
    async fn execute(
        &self,
        assignment: Assignment,
        cancel: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        if cancel.is_cancelled() {
            return Err(RuntimeError::Cancelled);
        }
        let mut result = AssignmentResult {
            sources: vec![], findings: vec![], uncertainties: vec!["These are invented fixture documents; no live research or quality assessment occurred.".into()],
            outline: vec![], draft: None, next: NextAction::Finish,
            usage: Usage { tool_calls: Some(0), ..Usage::default() },
        };
        match assignment.kind {
            AssignmentKind::Investigate => {
                result.sources = self.sources.clone();
                result.findings = self
                    .sources
                    .iter()
                    .map(|s| Finding {
                        text: s.excerpt.clone(),
                        sources: vec![s.id.clone()],
                    })
                    .collect();
                result.outline = match assignment.strategy {
                    Strategy::Focused => vec!["Answer".into(), "Remaining uncertainty".into()],
                    Strategy::Exploration => vec![
                        "Durable execution".into(),
                        "Simple execution".into(),
                        "Recovery tradeoffs".into(),
                    ],
                    Strategy::Comparison => vec![
                        "Criteria: recovery and complexity".into(),
                        "Alternatives".into(),
                        "Decision-changing evidence".into(),
                    ],
                    Strategy::Collection => vec![
                        "Candidates".into(),
                        "Known attributes".into(),
                        "Missing attributes and scope".into(),
                    ],
                };
                result.next = NextAction::Synthesize;
            }
            AssignmentKind::Synthesize | AssignmentKind::CompleteResearch => {
                if assignment.kind == AssignmentKind::CompleteResearch {
                    result.sources = self.sources.clone();
                }
                let mut draft = format!(
                    "# Fixture research\n\nQuestion: {}\n\n",
                    assignment.objective
                );
                if assignment.format == OutputFormat::Table {
                    draft.push_str("| Candidate | Evidence |\n|---|---|\n");
                    for source in &self.sources {
                        draft.push_str(&format!(
                            "| {} | {} [{}] |\n",
                            source.title, source.excerpt, source.id
                        ));
                    }
                } else {
                    for source in &self.sources {
                        draft.push_str(&format!("{} [{}]\n\n", source.excerpt, source.id));
                    }
                }
                draft.push_str("External process recovery still requires investigation. These fixture alternatives are not product recommendations.");
                result.draft = Some(draft);
            }
            AssignmentKind::Review => (),
        }
        Ok(result)
    }
}
