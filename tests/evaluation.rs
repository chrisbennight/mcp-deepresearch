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
