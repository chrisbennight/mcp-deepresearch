//! Scripted decisions establish controller behavior, not a model's research quality.
use mcp_deepresearch::{
    controller::Controller,
    research::*,
    runtime::{self, AgentRuntime, FixtureRuntime, RuntimeError},
    workspace::Workspace,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

struct Investigation {
    seen: Mutex<Vec<Strategy>>,
}
impl AgentRuntime for Investigation {
    async fn execute(
        &self,
        assignment: Assignment,
        cancel: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        self.seen.lock().await.push(assignment.strategy);
        let mut result = FixtureRuntime::default()
            .execute(assignment.clone(), cancel)
            .await?;
        if assignment.kind == AssignmentKind::Investigate {
            match assignment.number {
                1 => {
                    result.sources.clear();
                    result.findings.clear();
                    result.uncertainties.push("The first query was fruitless; its snippet is not evidence. Two reposts repeat the same unsupported claim.".into());
                    result.next = NextAction::Investigate { question: "Explore the missing external-process recovery category in primary documentation.".into(), strategy: Some(Strategy::Exploration) };
                }
                2 => {
                    result.sources[0].excerpt = "The primary guide says workflow steps recover, but an external process does not automatically resume.".into();
                    result.sources[1].excerpt = "The older guide claims all work resumes automatically; its applicability is unresolved.".into();
                    result.findings = vec![Finding { text: "The guides disagree about external recovery; neither duplicate snippets nor document counts settle this.".into(), sources:vec!["S1".into(),"S2".into()] }];
                    result.uncertainties.push("External-process recovery remains disputed; do not present it as guaranteed.".into());
                    result.next = NextAction::Investigate {
                        question: "Return to the decision using the discovered distinction.".into(),
                        strategy: Some(Strategy::Comparison),
                    };
                }
                _ => {
                    result.sources.clear();
                    result.findings.clear();
                    result.next = NextAction::Synthesize;
                }
            }
        } else if assignment.kind == AssignmentKind::Synthesize {
            result.draft = Some("# Fixture decision\nWorkflow recovery does not establish external-process recovery [S1]. The older contrary claim remains unresolved [S2]. Duplicate reporting is not independent support; the initial query produced no usable evidence.".into());
        }
        Ok(result)
    }
}
#[tokio::test]
async fn an_unproductive_query_can_change_strategy_and_preserve_a_consequential_contradiction() {
    let request = serde_json::from_value(
        serde_json::json!({"objective":"Compare recovery approaches", "strategy":"comparison"}),
    )
    .unwrap();
    let mut controller = Controller::new(
        Workspace::new("fixture".into(), request).unwrap(),
        runtime::unix_seconds(),
    );
    let agent = Investigation {
        seen: Mutex::new(Vec::new()),
    };
    runtime::run(&agent, &mut controller, CancellationToken::new()).await;
    assert_eq!(controller.workspace.status, Status::Completed);
    let seen = agent.seen.lock().await;
    assert_eq!(
        &seen[..3],
        &[
            Strategy::Comparison,
            Strategy::Exploration,
            Strategy::Comparison
        ]
    );
    let report = controller.workspace.report();
    assert!(report.contains("initial query produced no usable evidence"));
    assert!(report.contains("older contrary claim remains unresolved"));
    assert!(report.contains("[S1]") && report.contains("[S2]"));
    assert!(
        controller.workspace.assignments_completed
            <= controller.workspace.request.limits.max_assignments
    );
}
