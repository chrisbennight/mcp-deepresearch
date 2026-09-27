use mcp_deepresearch::{
    controller::Controller,
    evaluation::{Case, compare},
    research::*,
    runtime::{self, AgentRuntime, FixtureRuntime, RuntimeError},
    workspace::Workspace,
};
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct ResearchFixture {
    inputs: Mutex<Vec<Assignment>>,
}
impl AgentRuntime for ResearchFixture {
    async fn execute(
        &self,
        assignment: Assignment,
        cancel: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        self.inputs.lock().unwrap().push(assignment.clone());
        let mut result = FixtureRuntime::default()
            .execute(assignment.clone(), cancel)
            .await?;
        result.usage.session_id = Some(format!(
            "fixture-{}-{}",
            assignment.research_id, assignment.number
        ));
        if assignment.kind == AssignmentKind::PrimaryResearch {
            result.uncertainties.push("Primary-only caveat".into());
            result.findings.push(Finding {
                text: "Primary-only finding".into(),
                sources: vec!["S1".into()],
            });
        }
        if assignment.kind == AssignmentKind::IndependentResearch {
            assert!(!assignment.context.contains("Primary-only finding"));
            result.questions = vec![ResearchQuestion {
                id: "Q-material".into(),
                question: "Does the material change the conclusion?".into(),
                important: true,
                answer: String::new(),
                sources: vec![],
                remaining_gap: "No material evidence yet".into(),
            }];
        }
        if assignment.kind == AssignmentKind::CoverageReview {
            assert!(assignment.context.contains("Primary-only finding"));
            assert!(assignment.context.contains("Primary-only caveat"));
            assert!(assignment.context.contains("Q-material"));
        }
        if assignment.kind == AssignmentKind::Review
            && !assignment.context.contains("Verified material finding")
        {
            result.next = NextAction::Investigate {
                question: "Find material evidence".into(),
                strategy: None,
            };
        }
        if assignment.kind == AssignmentKind::Investigate {
            result.sources.push(Source {
                id: "S3".into(),
                url: "https://example.org/material".into(),
                title: "Material evidence".into(),
                excerpt: "Verified material finding".into(),
                needs_refresh: false,
            });
            result.findings.push(Finding {
                text: "Verified material finding".into(),
                sources: vec!["S3".into()],
            });
        }
        if assignment.kind == AssignmentKind::Synthesize
            && assignment.context.contains("Verified material finding")
        {
            result.draft = Some("Revised answer incorporates the material constraint [S3].".into());
        }
        Ok(result)
    }
}
fn request() -> ResearchRequest {
    serde_json::from_value(serde_json::json!({"objective":"Compare recovery approaches and important constraints", "policy":"multi_agent", "limits":{"max_assignments":12}})).unwrap()
}

#[tokio::test]
async fn independent_research_and_review_repair_change_the_delivered_answer() {
    let runtime = ResearchFixture::default();
    let mut controller = Controller::new(
        Workspace::new("test".into(), request()).unwrap(),
        runtime::unix_seconds(),
    );
    // Simulate durable controller delivery across every handoff.
    while let Some(assignment) = controller.assignment(runtime::unix_seconds()) {
        let result = runtime
            .execute(assignment, CancellationToken::new())
            .await
            .unwrap();
        controller.complete(result).unwrap();
        controller = serde_json::from_slice(&serde_json::to_vec(&controller).unwrap()).unwrap();
    }
    assert_eq!(controller.workspace.status, Status::Completed);
    assert!(
        controller
            .workspace
            .draft
            .contains("material constraint [S3]")
    );
    assert!(controller.workspace.questions.contains_key("Q-material"));
    let inputs = runtime.inputs.lock().unwrap();
    assert!(
        inputs
            .iter()
            .any(|a| a.kind == AssignmentKind::Investigate && a.focus == "Find material evidence")
    );
    let review = inputs.last().unwrap();
    assert_eq!(review.kind, AssignmentKind::Review);
    assert!(review.context.contains(&controller.workspace.draft));
}

#[tokio::test]
async fn exhaustion_cannot_masquerade_as_reviewed_completion() {
    let mut req = request();
    req.limits.max_assignments = 3;
    let mut controller = Controller::new(
        Workspace::new("test".into(), req).unwrap(),
        runtime::unix_seconds(),
    );
    runtime::run(
        &ResearchFixture::default(),
        &mut controller,
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(
        controller.workspace.status,
        Status::Exhausted { .. }
    ));
    assert!(
        controller
            .workspace
            .report()
            .contains("Primary-only finding")
    );
}

#[tokio::test]
async fn three_arm_comparison_requires_observed_sessions_for_multi_agent_claims() {
    let root = std::env::temp_dir().join(format!("inquiry-eval-{}", ResearchId::default()));
    let case = || Case {
        id: "research".into(),
        single_session_policy: false,
        baseline_first: false,
        request: request(),
        assess: vec![],
    };
    let evaluation = compare(
        &ResearchFixture::default(),
        vec![case()],
        "fixture",
        &root,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(evaluation.measurements.len(), 3);
    let multi = evaluation
        .measurements
        .iter()
        .find(|m| m.arm == "multi_agent")
        .unwrap();
    assert_eq!(multi.workflow_verified, Some(true));
    for arm in ["single_session", "question_driven"] {
        let m = evaluation
            .measurements
            .iter()
            .find(|m| m.arm == arm)
            .unwrap();
        assert_eq!(m.runtime_calls, 1);
        assert_eq!(m.workflow_verified, None);
    }
    let no_sessions = compare(
        &FixtureRuntime::default(),
        vec![case()],
        "fixture",
        &root,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        no_sessions
            .measurements
            .iter()
            .find(|m| m.arm == "multi_agent")
            .unwrap()
            .workflow_verified,
        Some(false)
    );
    let mut invalid = case();
    invalid.single_session_policy = true;
    assert!(
        compare(
            &ResearchFixture::default(),
            vec![invalid],
            "fixture",
            &root,
            CancellationToken::new()
        )
        .await
        .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}
