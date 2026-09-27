use mcp_deepresearch::{
    evaluation::{Case, compare},
    research::*,
    runtime::{AgentRuntime, FixtureRuntime, RuntimeError},
};
use tokio_util::sync::CancellationToken;
struct Unavailable;
impl AgentRuntime for Unavailable {
    async fn execute(
        &self,
        _: Assignment,
        _: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        Err(RuntimeError::Failed("fixture source unavailable".into()))
    }
}
fn case() -> Case {
    Case {
        id: "contract".into(),
        request: serde_json::from_value(
            serde_json::json!({"objective":"Compare the fixture alternatives"}),
        )
        .unwrap(),
        assess: vec!["No quality claim from execution".into()],
    }
}
#[tokio::test]
async fn comparison_distinguishes_answers_failures_and_skipped_work() {
    let root = std::env::temp_dir().join(format!("research-evaluation-{}", ResearchId::default()));
    let completed = compare(
        &FixtureRuntime::default(),
        vec![case()],
        "fixture",
        &root,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(
        completed
            .measurements
            .iter()
            .all(|r| r.outcome == "completed" && r.quality_score.is_none())
    );
    assert_eq!(
        completed
            .measurements
            .iter()
            .find(|r| r.arm == "single_session")
            .unwrap()
            .runtime_calls,
        1
    );
    let failed = compare(
        &Unavailable,
        vec![case()],
        "fixture",
        &root,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(
        failed
            .measurements
            .iter()
            .all(|r| r.outcome == "failed" && r.usage.tool_calls.is_none())
    );
    let cancel = CancellationToken::new();
    cancel.cancel();
    let skipped = compare(
        &FixtureRuntime::default(),
        vec![case()],
        "fixture",
        &root,
        cancel,
    )
    .await
    .unwrap();
    assert!(
        skipped
            .measurements
            .iter()
            .all(|r| r.outcome == "skipped" && r.runtime_calls == 0)
    );
    std::fs::remove_dir_all(root).unwrap();
}

struct PartialDraft(NextAction);
impl AgentRuntime for PartialDraft {
    async fn execute(
        &self,
        _: Assignment,
        _: CancellationToken,
    ) -> Result<AssignmentResult, RuntimeError> {
        Ok(AssignmentResult {
            sources: vec![],
            findings: vec![],
            uncertainties: vec![],
            outline: vec![],
            draft: Some("Partial answer; important work remains.".into()),
            next: self.0.clone(),
            usage: Usage::default(),
        })
    }
}
#[tokio::test]
async fn baseline_partial_drafts_do_not_hide_clarification_or_unfinished_work() {
    let root = std::env::temp_dir().join(format!("research-evaluation-{}", ResearchId::default()));
    for (action, expected) in [
        (
            NextAction::AskUser {
                question: "Which alternative matters?".into(),
            },
            "needs_input",
        ),
        (
            NextAction::Investigate {
                question: "Read the primary evidence".into(),
                strategy: None,
            },
            "incomplete",
        ),
    ] {
        let result = compare(
            &PartialDraft(action),
            vec![case()],
            "fixture",
            &root,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let baseline = result
            .measurements
            .iter()
            .find(|r| r.arm == "single_session")
            .unwrap();
        assert_eq!(baseline.outcome, expected);
        assert!(
            std::fs::read_to_string(root.join(format!("{}.md", baseline.research_id)))
                .unwrap()
                .contains("Partial answer")
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
