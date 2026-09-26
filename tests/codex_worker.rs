use mcp_deepresearch::{
    codex::{CodexConfig, CodexRuntime},
    research::*,
    runtime::{AgentRuntime, RuntimeError},
};
use std::{path::PathBuf, time::Duration};
use tokio_util::sync::CancellationToken;

fn config(script: &str) -> CodexConfig {
    let root = std::env::temp_dir().join(format!("codex-worker-test-{}", ResearchId::default()));
    CodexConfig {
        executable: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(script),
        auth_home: root.join("empty-auth"),
        work_root: root,
        sources: None,
        model: None,
        max_workers: 1,
    }
}
fn assignment() -> Assignment {
    Assignment {
        attachments: Vec::new(),
        deadline_unix_seconds: None,
        trace_context: TraceContext::default(),
        research_id: ResearchId::default(),
        number: 1,
        kind: AssignmentKind::Investigate,
        objective: "Test worker behavior".into(),
        focus: "Use fixture sources".into(),
        strategy: Strategy::Focused,
        format: OutputFormat::Answer,
        context: String::new(),
        remaining_seconds: 5,
        remaining_tool_calls: 5,
    }
}

#[tokio::test]
async fn reconnect_and_restart_reuse_completed_work_without_duplicate_processes() {
    let config = config("codex-success.sh");
    let runtime = CodexRuntime::new(config.clone()).unwrap();
    let assignment = assignment();
    let (first, second) = tokio::join!(
        runtime.execute(assignment.clone(), CancellationToken::new()),
        runtime.execute(assignment.clone(), CancellationToken::new())
    );
    assert_eq!(first.unwrap().draft, second.unwrap().draft);
    let restarted = CodexRuntime::new(config.clone()).unwrap();
    let result = restarted
        .execute(assignment.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.usage.tool_calls, Some(1));
    assert_eq!(result.usage.input_tokens, Some(12));
    let dir = config
        .work_root
        .join(format!("{}-1", assignment.research_id));
    assert_eq!(
        std::fs::read_to_string(dir.join("launches.txt")).unwrap(),
        "started\n"
    );
    assert_eq!(
        restarted
            .inspect(assignment.research_id, 1)
            .await
            .unwrap()
            .session_id
            .as_deref(),
        Some("fixture-session")
    );
    std::fs::remove_dir_all(config.work_root).unwrap();
}

#[tokio::test]
async fn interrupted_assignment_is_reported_without_automatically_starting_another_process() {
    let config = config("codex-success.sh");
    let assignment = assignment();
    let dir = config
        .work_root
        .join(format!("{}-1", assignment.research_id));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("worker.json"),
        r#"{"session_id":"interrupted-session","tool_calls":1,"outcome":null}"#,
    )
    .unwrap();
    let runtime = CodexRuntime::new(config.clone()).unwrap();
    let error = runtime
        .execute(assignment, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("interrupted"));
    assert!(!dir.join("launches.txt").exists());
    std::fs::remove_dir_all(config.work_root).unwrap();
}

#[tokio::test]
async fn cancellation_stops_the_running_process_group_before_acknowledgment() {
    let config = config("codex-slow.sh");
    let runtime = CodexRuntime::new(config.clone()).unwrap();
    let assignment = assignment();
    let id = assignment.research_id;
    let token = CancellationToken::new();
    let cancel = token.clone();
    let worker_runtime = runtime.clone();
    let running = tokio::spawn(async move { worker_runtime.execute(assignment, token).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if runtime
                .inspect(id, 1)
                .await
                .is_ok_and(|s| s.session_id.is_some())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    cancel.cancel();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(3), running)
            .await
            .unwrap()
            .unwrap(),
        Err(RuntimeError::Cancelled)
    ));
    let dir = config.work_root.join(format!("{id}-1"));
    for file in ["parent.pid", "descendant.pid"] {
        let pid = std::fs::read_to_string(dir.join(file)).unwrap();
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{}/stat", pid.trim())) {
            // A reparented zombie has stopped executing, even before init reaps it.
            assert_eq!(stat.split_whitespace().nth(2), Some("Z"));
        }
    }
    std::fs::remove_dir_all(config.work_root).unwrap();
}

#[tokio::test]
async fn assignment_deadline_stops_a_nonresponsive_worker() {
    let config = config("codex-slow.sh");
    let runtime = CodexRuntime::new(config.clone()).unwrap();
    let mut assignment = assignment();
    assignment.remaining_seconds = 1;
    let outcome = tokio::time::timeout(
        Duration::from_secs(3),
        runtime.execute(assignment, CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Err(RuntimeError::TimedOut)));
    std::fs::remove_dir_all(config.work_root).unwrap();
}

#[tokio::test]
async fn malformed_output_and_crash_after_output_do_not_trigger_blind_retries() {
    for mode in ["fixture-malformed", "fixture-crash"] {
        let mut config = config("codex-success.sh");
        config.model = Some(mode.into());
        let assignment = assignment();
        let runtime = CodexRuntime::new(config.clone()).unwrap();
        assert!(
            runtime
                .execute(assignment.clone(), CancellationToken::new())
                .await
                .is_err()
        );
        let restarted = CodexRuntime::new(config.clone()).unwrap();
        assert!(
            restarted
                .execute(assignment.clone(), CancellationToken::new())
                .await
                .is_err()
        );
        let dir = config
            .work_root
            .join(format!("{}-1", assignment.research_id));
        assert!(dir.join("answer.json").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("launches.txt")).unwrap(),
            "started\n"
        );
        std::fs::remove_dir_all(config.work_root).unwrap();
    }
}

#[tokio::test]
async fn cancellation_before_delivery_prevents_a_late_worker_launch() {
    let config = config("codex-success.sh");
    let runtime = CodexRuntime::new(config.clone()).unwrap();
    let assignment = assignment();
    runtime
        .stop_if_started(assignment.research_id, assignment.number)
        .await
        .unwrap();
    assert!(matches!(
        runtime
            .execute(assignment.clone(), CancellationToken::new())
            .await,
        Err(RuntimeError::Cancelled)
    ));
    assert!(
        !config
            .work_root
            .join(format!("{}-{}", assignment.research_id, assignment.number))
            .join("launches.txt")
            .exists()
    );
    std::fs::remove_dir_all(config.work_root).unwrap();
}

#[tokio::test]
async fn an_expired_absolute_deadline_never_launches_a_worker() {
    let mut assignment = assignment();
    assignment.deadline_unix_seconds = Some(1);
    let config = config("codex-success.sh");
    let directory = config
        .work_root
        .join(format!("{}-1", assignment.research_id));
    let runtime = CodexRuntime::new(config.clone()).unwrap();
    assert!(matches!(
        runtime.execute(assignment, CancellationToken::new()).await,
        Err(RuntimeError::TimedOut)
    ));
    assert!(!directory.join("launches.txt").exists());
    std::fs::remove_dir_all(config.work_root).unwrap();
}
