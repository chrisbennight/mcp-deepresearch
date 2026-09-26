use mcp_deepresearch::{research::*, workspace::*};

fn request() -> ResearchRequest {
    serde_json::from_str(r#"{"objective":"Compare durability choices"}"#).unwrap()
}
fn result() -> AssignmentResult {
    AssignmentResult {
        sources: vec![Source {
            id: "S1".into(),
            url: "https://example.org/durability".into(),
            title: "Durability".into(),
            excerpt: "Durable workflows resume completed steps.".into(),
            needs_refresh: false,
        }],
        findings: vec![Finding {
            text: "Recovery can reuse finished work.".into(),
            sources: vec!["S1".into()],
        }],
        uncertainties: vec!["Check external process recovery.".into()],
        outline: vec![],
        draft: Some("Recovery reuses completed work [S1].".into()),
        next: NextAction::Finish,
        usage: Usage::default(),
    }
}
#[test]
fn persisted_revision_reuses_evidence_without_sharing_writes() {
    let root = std::env::temp_dir().join(format!("research-test-{}", ResearchId::default()));
    let store = WorkspaceStore::new(&root).unwrap();
    let mut original = Workspace::new("alice".into(), request()).unwrap();
    original.apply(result()).unwrap();
    original.status = Status::Completed;
    store.save(&original).unwrap();
    assert!(store.load(original.id, "bob").is_err());
    let restored = store.load(original.id, "alice").unwrap();
    let mut revision = restored.revise("alice", request()).unwrap();
    assert_ne!(revision.id, original.id);
    revision.draft = "New draft".into();
    store.save(&revision).unwrap();
    assert_eq!(
        store.load(original.id, "alice").unwrap().draft,
        original.draft
    );
    assert!(revision.sources["S1"].needs_refresh);
    assert!(
        revision
            .context("durability")
            .contains("recheck time-sensitive facts")
    );
    assert!(revision.report().contains("require rechecking"));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn rejects_unsupported_citations_without_partially_updating_notes() {
    let mut workspace = Workspace::new("alice".into(), request()).unwrap();
    let mut unsupported = result();
    unsupported.draft = Some("An invented conclusion [S99].".into());
    assert!(workspace.apply(unsupported).is_err());
    assert!(workspace.notes.is_empty());
    assert!(workspace.sources.is_empty());
    workspace.apply(result()).unwrap();
    assert!(
        workspace
            .report()
            .contains("[S1]: https://example.org/durability")
    );
}
#[test]
fn cancelled_work_rejects_late_results_and_preserves_partial_answer() {
    let mut workspace = Workspace::new("alice".into(), request()).unwrap();
    workspace.apply(result()).unwrap();
    workspace.status = Status::Cancelled;
    assert!(workspace.apply(result()).is_err());
    assert_eq!(workspace.assignments_completed, 1);
    assert!(workspace.report().contains("partial material"));
}
#[test]
fn context_is_bounded_and_selects_relevant_evidence() {
    let mut workspace = Workspace::new("alice".into(), request()).unwrap();
    workspace.apply(result()).unwrap();
    workspace.sources.insert(
        "S2".into(),
        Source {
            id: "S2".into(),
            url: "https://example.org/unrelated".into(),
            title: "Unrelated".into(),
            excerpt: "界".repeat(50_000),
            needs_refresh: false,
        },
    );
    let context = workspace.context("durability");
    assert!(context.chars().count() <= workspace.request.limits.context_chars);
    assert!(context.contains("Durable workflows"));
    assert!(context.contains("[truncated]"));
    assert!(context.contains("may be omitted"));
    assert!(!workspace.tool_usage_complete);
}
#[test]
fn invalid_requests_fail_before_work_starts() {
    let mut req = request();
    req.limits.max_assignments = 0;
    assert!(Workspace::new("alice".into(), req).is_err());
}

#[test]
fn unfinished_report_includes_evidence_gathered_after_the_draft() {
    let mut workspace = Workspace::new("alice".into(), request()).unwrap();
    workspace.apply(result()).unwrap();
    let mut followup = result();
    followup.draft = None;
    followup.findings[0].text = "New evidence changes the recommendation.".into();
    workspace.apply(followup).unwrap();
    workspace.status = Status::Exhausted {
        reason: "source-tool limit".into(),
    };
    let report = workspace.report();
    assert!(report.contains("New evidence changes the recommendation."));
    assert!(report.contains("may predate the latest evidence"));
}
