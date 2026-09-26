//! Restate owns execution; the controller owns research decisions and material.
use crate::{
    codex::CodexRuntime,
    controller::Controller,
    research::*,
    runtime::{self, AgentRuntime, FixtureRuntime, RuntimeError},
    workspace::Workspace,
};
use restate_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub enum Worker {
    Codex(CodexRuntime),
    Fixture {
        delay: Duration,
        clarification: bool,
    },
}
impl Worker {
    async fn execute(
        &self,
        assignment: Assignment,
        cancel: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        match self {
            Self::Codex(worker) => worker.execute(assignment, cancel).await,
            Self::Fixture {
                delay,
                clarification,
            } => {
                let remaining = assignment.time_remaining();
                if remaining.is_zero() {
                    return Err(RuntimeError::TimedOut);
                }
                tokio::select! {
                    _ = tokio::time::sleep(remaining) => return Err(RuntimeError::TimedOut),
                    _ = cancel.cancelled() => return Err(RuntimeError::Cancelled),
                    _ = tokio::time::sleep(*delay) => (),
                }
                let ask = *clarification && assignment.number == 1;
                let mut result = FixtureRuntime::default()
                    .execute(assignment, cancel)
                    .await?;
                if ask {
                    result.next = NextAction::AskUser {
                        question: "Which decision criterion matters most?".into(),
                    };
                }
                Ok(result)
            }
        }
    }
    async fn stop(&self, id: ResearchId, number: u32) -> Result<(), RuntimeError> {
        if let Self::Codex(worker) = self {
            worker.stop_if_started(id, number).await?;
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Submission {
    pub attachments: Vec<Attachment>,
    pub trace_context: TraceContext,
    pub id: ResearchId,
    pub owner: String,
    pub request: ResearchRequest,
    pub created_at: u64,
    pub previous: Option<Workspace>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ResearchState {
    pub controller: Controller,
    pub stage: String,
    pub updated_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Owner {
    pub principal: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct UserInput {
    pub principal: String,
    pub question_id: u32,
    pub response: Clarification,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Clarification {
    Answer(String),
    Cancel,
}

pub fn workflow_key(owner: &str, id: ResearchId) -> String {
    format!("{owner}:{id}")
}

#[derive(Clone)]
pub struct Research {
    worker: Arc<Worker>,
}
impl Research {
    pub fn new(worker: Worker) -> Self {
        Self {
            worker: Arc::new(worker),
        }
    }
}

fn terminal(error: impl std::fmt::Display) -> TerminalError {
    TerminalError::new_with_code(400, error.to_string())
}
fn verify_key(key: &str, principal: &str) -> Result<ResearchId, TerminalError> {
    let (owner, id) = key
        .rsplit_once(':')
        .ok_or_else(|| terminal("invalid research identity"))?;
    if owner != principal {
        return Err(TerminalError::new_with_code(404, "research not found"));
    }
    Ok(ResearchId(
        uuid::Uuid::parse_str(id).map_err(|_| terminal("invalid research identity"))?,
    ))
}

#[workflow(workflow_completion_retention = "7 days")]
impl Research {
    #[handler]
    async fn run(
        &self,
        ctx: WorkflowContext<'_>,
        submission: Json<Submission>,
    ) -> HandlerResult<Json<Workspace>> {
        let submission = submission.into_inner();
        if verify_key(ctx.key(), &submission.owner)? != submission.id {
            return Err(terminal("request identity mismatch").into());
        }
        submission.request.validate().map_err(terminal)?;
        let mut workspace = if let Some(previous) = submission.previous {
            previous
                .revise(&submission.owner, submission.request)
                .map_err(terminal)?
        } else {
            Workspace::new(submission.owner, submission.request).map_err(terminal)?
        };
        workspace.id = submission.id;
        workspace.attachments = submission.attachments;
        for (index, attachment) in workspace.attachments.iter().enumerate() {
            let id = format!("S{}", 1_000_000 + index);
            workspace
                .sources
                .entry(id.clone())
                .or_insert_with(|| Source {
                    id,
                    url: attachment.uri.clone(),
                    title: attachment.name.clone(),
                    excerpt: attachment.excerpt.clone(),
                    needs_refresh: false,
                });
        }
        let mut state = ResearchState {
            controller: Controller::new(workspace, submission.created_at),
            stage: "queued".into(),
            updated_at: submission.created_at,
        };
        state.controller.trace_context = submission.trace_context;
        ctx.set("research", Json(state.clone()));
        let deadline = state
            .controller
            .started_at
            .saturating_add(state.controller.workspace.request.limits.wall_seconds);
        loop {
            let now = ctx
                .run(|| async { Ok(runtime::unix_seconds()) })
                .name("observe elapsed time")
                .await?;
            state.updated_at = now;
            if state.controller.workspace.status.terminal() {
                break;
            }
            if ctx.peek_promise::<bool>("cancel").await?.is_some() {
                state.controller.workspace.status = Status::Cancelled;
            }
            if state.controller.workspace.status.terminal() {
                break;
            }
            let remaining = state
                .controller
                .workspace
                .request
                .limits
                .wall_seconds
                .saturating_sub(now.saturating_sub(state.controller.started_at));
            if remaining == 0 {
                state.controller.workspace.status = Status::Exhausted {
                    reason: "wall-clock limit".into(),
                };
                break;
            }
            if matches!(
                state.controller.workspace.status,
                Status::InputRequired { .. }
            ) {
                state.stage = "waiting for clarification".into();
                ctx.set("research", Json(state.clone()));
                let name = format!("input-{}", state.controller.workspace.assignments_completed);
                let answer = restate_sdk::select! {
                    answer=ctx.promise::<Json<Clarification>>(&name) => Some(answer?),
                    cancelled=ctx.promise::<bool>("cancel") => { cancelled?; state.controller.workspace.status=Status::Cancelled;None },
                    timer=ctx.sleep(time_until(deadline)) => { timer?;state.controller.workspace.status=Status::Exhausted{reason:"wall-clock limit while awaiting input".into()};None },
                };
                if let Some(answer) = answer {
                    match answer.into_inner() {
                        Clarification::Answer(answer) => {
                            state.controller.provide_input(&answer).map_err(terminal)?
                        }
                        Clarification::Cancel => {
                            state.controller.workspace.status = Status::Cancelled
                        }
                    }
                }
                continue;
            }
            let Some(assignment) = state.controller.assignment(now) else {
                break;
            };
            state.stage = match assignment.kind {
                AssignmentKind::Investigate => "investigate",
                AssignmentKind::Synthesize => "write",
                AssignmentKind::Review => "review",
            }
            .into();
            ctx.set("research", Json(state.clone()));
            let execution = ctx
                .service_client::<AssignmentsClient>()
                .execute(Json(assignment.clone()))
                .call();
            let outcome = restate_sdk::select! {
                result=execution => Some(result?.into_inner()),
                cancelled=ctx.promise::<bool>("cancel") => { cancelled?;None },
                timer=ctx.sleep(time_until(deadline)) => { timer?;Some(Err(RuntimeError::TimedOut)) },
                on_cancel => { return Err(TerminalError::new("research invocation interrupted").into()); }
            };
            if outcome.is_none() || matches!(outcome, Some(Err(RuntimeError::TimedOut))) {
                let worker = self.worker.clone();
                let stopped = ctx
                    .run(move || {
                        let worker = worker.clone();
                        async move {
                            Ok(Json(
                                worker.stop(assignment.research_id, assignment.number).await,
                            ))
                        }
                    })
                    .name("stop active worker")
                    .await?
                    .into_inner();
                if let Err(error) = stopped {
                    state.controller.workspace.status = Status::Failed {
                        reason: error.to_string(),
                    };
                    break;
                }
            }
            // A cancellation resolved during result delivery wins before notes are applied.
            if ctx.peek_promise::<bool>("cancel").await?.is_some() {
                state.controller.workspace.status = Status::Cancelled;
                break;
            }
            let delivered_at = ctx
                .run(|| async { Ok(runtime::unix_seconds()) })
                .name("observe result delivery time")
                .await?;
            state.updated_at = delivered_at;
            // Preserve accepted material when an assignment arrives after its deadline.
            if delivered_at >= deadline {
                state.controller.workspace.status = Status::Exhausted {
                    reason: "wall-clock limit".into(),
                };
                break;
            }
            match outcome {
                Some(Ok(result)) => {
                    if let Err(error) = state.controller.complete(result) {
                        state.controller.workspace.status = Status::Failed {
                            reason: error.to_string(),
                        };
                    }
                }
                None | Some(Err(RuntimeError::Cancelled)) => {
                    state.controller.workspace.status = Status::Cancelled
                }
                Some(Err(RuntimeError::TimedOut)) => {
                    state.controller.workspace.status = Status::Exhausted {
                        reason: "wall-clock limit".into(),
                    }
                }
                Some(Err(error)) => {
                    state.controller.workspace.status = Status::Failed {
                        reason: error.to_string(),
                    }
                }
            }
            ctx.set("research", Json(state.clone()));
        }
        state.stage = "finished".into();
        ctx.set("research", Json(state.clone()));
        Ok(Json(state.controller.workspace))
    }

    #[handler]
    async fn status(
        &self,
        ctx: SharedWorkflowContext<'_>,
        owner: Json<Owner>,
    ) -> HandlerResult<Json<Option<ResearchState>>> {
        verify_key(ctx.key(), &owner.0.principal)?;
        Ok(Json(
            ctx.get::<Json<ResearchState>>("research")
                .await?
                .map(Json::into_inner),
        ))
    }

    #[handler]
    async fn cancel(
        &self,
        ctx: SharedWorkflowContext<'_>,
        owner: Json<Owner>,
    ) -> HandlerResult<()> {
        verify_key(ctx.key(), &owner.0.principal)?;
        if let Some(state) = ctx.get::<Json<ResearchState>>("research").await?
            && state.0.controller.workspace.status.terminal()
        {
            return Ok(());
        }
        ctx.resolve_promise("cancel", true);
        Ok(())
    }

    #[handler]
    async fn provide_input(
        &self,
        ctx: SharedWorkflowContext<'_>,
        input: Json<UserInput>,
    ) -> HandlerResult<()> {
        let input = input.into_inner();
        verify_key(ctx.key(), &input.principal)?;
        let mut state = ctx
            .get::<Json<ResearchState>>("research")
            .await?
            .ok_or_else(|| TerminalError::new_with_code(404, "research not found"))?
            .into_inner();
        if state.controller.workspace.assignments_completed != input.question_id
            || !matches!(
                state.controller.workspace.status,
                Status::InputRequired { .. }
            )
        {
            return Ok(());
        }
        let name = format!("input-{}", input.question_id);
        if ctx
            .peek_promise::<Json<Clarification>>(&name)
            .await?
            .is_some()
        {
            return Ok(());
        }
        if let Clarification::Answer(answer) = &input.response {
            state.controller.provide_input(answer).map_err(terminal)?;
        }
        // The durable promise chooses one response. A delayed response can only
        // resolve this question, never cancel a later stage or another question.
        ctx.resolve_promise(&name, Json(input.response));
        ctx.promise::<Json<Clarification>>(&name).await?;
        Ok(())
    }
}

/// Meaningful assignment calls appear individually in Restate's execution view.
#[derive(Clone)]
pub struct Assignments {
    worker: Arc<Worker>,
}
impl Assignments {
    pub fn new(worker: Worker) -> Self {
        Self {
            worker: Arc::new(worker),
        }
    }
}
#[restate_sdk::service]
impl Assignments {
    #[handler]
    async fn execute(
        &self,
        ctx: Context<'_>,
        assignment: Json<Assignment>,
    ) -> HandlerResult<Json<Result<AssignmentResult, RuntimeError>>> {
        let worker = self.worker.clone();
        let assignment = assignment.into_inner();
        let step_name = format!("{:?} assignment {}", assignment.kind, assignment.number);
        ctx.run(move || {
            let worker = worker.clone();
            let assignment = assignment.clone();
            async move {
                Ok(Json(
                    worker.execute(assignment, CancellationToken::new()).await,
                ))
            }
        })
        .name(step_name)
        .await
        .map_err(Into::into)
    }
}
