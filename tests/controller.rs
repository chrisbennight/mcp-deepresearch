use mcp_deepresearch::{
    controller::Controller,
    research::*,
    runtime::{self, AgentRuntime, FixtureRuntime, RuntimeError},
    workspace::Workspace,
};
use tokio_util::sync::CancellationToken;

fn controller(strategy: Strategy) -> Controller {
    let mut req: ResearchRequest =
        serde_json::from_str(r#"{"objective":"Compare recovery approaches"}"#).unwrap();
    req.strategy = strategy;
    Controller::new(
        Workspace::new("operator".into(), req).unwrap(),
        runtime::unix_seconds(),
    )
}

#[tokio::test]
async fn all_strategies_produce_cited_fixture_answers_through_shared_execution() {
    for strategy in [
        Strategy::Focused,
        Strategy::Exploration,
        Strategy::Comparison,
        Strategy::Collection,
    ] {
        let mut control = controller(strategy);
        runtime::run(
            &FixtureRuntime::default(),
            &mut control,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(control.workspace.status, Status::Completed);
        assert!(control.workspace.report().contains("[S1]"));
        assert!(control.workspace.report().contains("invented fixture"));
        assert!(!control.workspace.outline.is_empty());
        assert!(!control.workspace.draft.contains("| Candidate |"));
    }
}

struct FollowUp;
impl AgentRuntime for FollowUp {
    async fn execute(
        &self,
        assignment: Assignment,
        cancel: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        let mut result = FixtureRuntime::default()
            .execute(assignment.clone(), cancel)
            .await?;
        if assignment.kind == AssignmentKind::Review {
            result.next = NextAction::Investigate {
                question: "Does external recovery change the conclusion?".into(),
                strategy: Some(Strategy::Focused),
            };
        }
        Ok(result)
    }
}
#[tokio::test]
async fn review_followups_are_bounded_and_explain_remaining_work() {
    let mut control = controller(Strategy::Comparison);
    runtime::run(&FollowUp, &mut control, CancellationToken::new()).await;
    assert_eq!(control.workspace.status, Status::Completed);
    assert_eq!(control.strategy, Strategy::Focused);
    assert!(
        control
            .workspace
            .report()
            .contains("bounded review allowance")
    );
}
#[tokio::test]
async fn assignment_exhaustion_preserves_a_partial_answer() {
    let mut control = controller(Strategy::Focused);
    control.workspace.request.limits.max_assignments = 2;
    runtime::run(
        &FixtureRuntime::default(),
        &mut control,
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(control.workspace.status, Status::Exhausted { .. }));
    assert!(control.workspace.report().contains("[S1]"));
    assert!(control.workspace.report().contains("assignment limit"));
}
#[tokio::test]
async fn cancellation_does_not_accept_late_results() {
    let mut control = controller(Strategy::Focused);
    let token = CancellationToken::new();
    struct LateResult(CancellationToken);
    impl AgentRuntime for LateResult {
        async fn execute(
            &self,
            assignment: Assignment,
            cancel: CancellationToken,
        ) -> Result<AssignmentResult, RuntimeError> {
            let result = FixtureRuntime::default()
                .execute(assignment, cancel)
                .await?;
            self.0.cancel();
            Ok(result)
        }
    }
    runtime::run(&LateResult(token.clone()), &mut control, token).await;
    assert_eq!(control.workspace.status, Status::Cancelled);
    assert_eq!(control.workspace.assignments_completed, 0);
}
#[test]
fn wall_clock_and_clarification_state_survive_serialization() {
    let mut control = controller(Strategy::Focused);
    control.workspace.status = Status::InputRequired {
        question: "Which criteria?".into(),
    };
    let mut restored: Controller =
        serde_json::from_slice(&serde_json::to_vec(&control).unwrap()).unwrap();
    assert!(restored.assignment(runtime::unix_seconds()).is_none());
    restored.provide_input("Prioritize recovery").unwrap();
    assert!(
        restored
            .workspace
            .request
            .context
            .contains("Prioritize recovery")
    );
    assert!(restored.assignment(restored.started_at + 10_000).is_none());
    assert!(matches!(
        restored.workspace.status,
        Status::Exhausted { .. }
    ));
}

#[test]
fn accepted_clarification_reaches_next_assignment_even_when_old_context_is_full() {
    let mut control = controller(Strategy::Comparison);
    control.workspace.request.limits.context_chars = 2000;
    control.workspace.request.context = "Old background. ".repeat(200);
    control.workspace.status = Status::InputRequired {
        question: "Which criteria matter?".into(),
    };
    control
        .provide_input("Prioritize recovery over latency.")
        .unwrap();
    let assignment = control.assignment(runtime::unix_seconds()).unwrap();
    assert!(
        assignment
            .focus
            .contains("Prioritize recovery over latency.")
    );
    assert!(assignment.context.chars().count() <= 2000);
}

#[tokio::test]
async fn cancellation_while_waiting_prevents_subsequent_clarification() {
    let mut control = controller(Strategy::Focused);
    control.workspace.status = Status::InputRequired {
        question: "Which criteria?".into(),
    };
    let cancel = CancellationToken::new();
    cancel.cancel();
    runtime::run(&FixtureRuntime::default(), &mut control, cancel).await;
    assert_eq!(control.workspace.status, Status::Cancelled);
    assert!(control.provide_input("Prioritize cost").is_err());
}
