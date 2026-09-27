//! A bounded assessment session through the same runtime as research.
use crate::{
    research::*,
    runtime::{AgentRuntime, unix_seconds},
};
use serde::Deserialize;
use std::path::Path;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub struct AssessmentRequest {
    pub objective: String,
    pub context: String,
    pub seconds: u64,
    pub tool_calls: u32,
}

pub async fn run<R: AgentRuntime>(
    runtime: &R,
    request: AssessmentRequest,
    output: &Path,
    cancel: CancellationToken,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.objective.trim().is_empty()
        || !(10..=7200).contains(&request.seconds)
        || request.tool_calls > 1000
    {
        return Err("assessment requires an objective and supported time/tool allowances".into());
    }
    let assignment = Assignment {
        policy: ResearchPolicy::Staged, attachments: vec![], trace_context: TraceContext::default(),
        deadline_unix_seconds: Some(unix_seconds() + request.seconds),
        research_id: ResearchId::default(), number: 1, kind: AssignmentKind::CompleteResearch,
        objective: request.objective, context: request.context,
        focus: "Assess the provided data. Return the requested JSON in draft and finish; do not rewrite the answer.".into(),
        strategy: Strategy::Focused, format: OutputFormat::Answer,
        remaining_seconds: request.seconds, remaining_tool_calls: request.tool_calls,
    };
    let result = runtime.execute(assignment, cancel).await?;
    let draft = result
        .draft
        .as_ref()
        .ok_or("assessment returned no draft")?;
    let parsed: serde_json::Value = serde_json::from_str(draft)?;
    std::fs::write(
        output.join("assessment.json"),
        serde_json::to_vec_pretty(&parsed)?,
    )?;
    std::fs::write(
        output.join("usage.json"),
        serde_json::to_vec_pretty(&result.usage)?,
    )?;
    Ok(())
}
