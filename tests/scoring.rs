use mcp_deepresearch::{evaluation::Evaluation, scoring::*};
use serde_json::json;

fn inputs(outcome: &str, errors: Vec<&str>) -> (ReferenceSuite, Evaluation, Judgments) {
    let id = "00000000-0000-0000-0000-000000000001";
    (
        serde_json::from_value(json!({"cases":[{"id":"question","split":"held_out","criteria":[{"id":"fact","expected":"Supported fact"}]}]})).unwrap(),
        serde_json::from_value(json!({"mode":"live","model":"test","source_tools":[],"note":"test","measurements":[{"case":"question","outcome":outcome,"arm":"single_session","research_id":id,"status":{"state":"completed"},"elapsed_ms":100,"runtime_calls":1,"usage":{},"monetary_cost":null,"human_interventions":0,"quality_score":null,"assess":[]}]})).unwrap(),
        serde_json::from_value(json!({"reviewer":"reviewer","method":"Checked source passages","answers":[{"research_id":id,"requirements":[{"id":"fact","score":2,"reason":"Answer gives the correct fact","evidence":"Primary source passage"}],"consequential_errors":errors}]})).unwrap(),
    )
}

#[test]
fn full_coverage_does_not_hide_consequential_errors_or_incomplete_execution() {
    for (outcome, errors, ready) in [
        ("completed", vec![], true),
        ("completed", vec!["Unsupported recommendation"], false),
        ("incomplete", vec![], false),
    ] {
        let (reference, evaluation, judgments) = inputs(outcome, errors);
        let report = score(reference, evaluation, judgments).unwrap();
        assert_eq!(report.answers[0].supported_requirements_percent, 100.0);
        assert_eq!(report.answers[0].decision_ready, ready);
        assert_eq!(report.answers[0].outcome, outcome);
    }
}

#[test]
fn missing_requirements_cannot_disappear_from_the_denominator() {
    let (reference, evaluation, mut judgments) = inputs("completed", vec![]);
    judgments.answers[0].requirements.clear();
    assert!(score(reference, evaluation, judgments).is_err());
}
