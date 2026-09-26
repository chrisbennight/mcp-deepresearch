//! Shared research concepts, independent of MCP transport and model provider.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    #[error("{0}")]
    Invalid(String),
    #[error("research is unavailable for this principal")]
    Unavailable,
    #[error("workspace storage failed")]
    Storage(#[from] std::io::Error),
    #[error("workspace data could not be decoded")]
    Encoding(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    #[default]
    Focused,
    Exploration,
    Comparison,
    Collection,
}

impl Strategy {
    pub fn guidance(self) -> &'static str {
        match self {
            Self::Focused => {
                "Resolve the focused question. Seek evidence that would disprove the apparent answer; follow only consequential gaps."
            }
            Self::Exploration => {
                "Discover distinct perspectives and useful questions. Revise the outline as evidence changes the field map; do not assume the initial categories are complete."
            }
            Self::Comparison => {
                "Use criteria tied to the user's decision. Investigate alternatives consistently and identify evidence that could change the recommendation. Distinguish missing evidence from a negative result."
            }
            Self::Collection => {
                "Track candidates, inclusion criteria, attributes, duplicates, and missing information. Look for omissions across sources; disclose the limits of the collection rather than claiming exhaustiveness."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Answer,
    Report,
    Table,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct Limits {
    pub max_assignments: u32,
    pub wall_seconds: u64,
    pub context_chars: usize,
    pub max_tool_calls: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_assignments: 8,
            wall_seconds: 900,
            context_chars: 32_000,
            max_tool_calls: 80,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResearchRequest {
    pub objective: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub strategy: Strategy,
    #[serde(default)]
    pub format: OutputFormat,
    #[serde(default)]
    pub source_constraints: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub limits: Limits,
}

impl ResearchRequest {
    pub fn validate(&self) -> Result<(), ResearchError> {
        if self.objective.trim().is_empty() || self.objective.chars().count() > 16_000 {
            return Err(ResearchError::Invalid(
                "objective must contain 1–16000 characters".into(),
            ));
        }
        if self.context.chars().count() > 16_000 {
            return Err(ResearchError::Invalid(
                "user context must fit within 16000 characters".into(),
            ));
        }
        if !(2..=32).contains(&self.limits.max_assignments)
            || !(10..=7200).contains(&self.limits.wall_seconds)
            || !(2000..=128_000).contains(&self.limits.context_chars)
            || !(1..=1000).contains(&self.limits.max_tool_calls)
        {
            return Err(ResearchError::Invalid(
                "research limits exceed the supported range".into(),
            ));
        }
        Ok(())
    }
}

/// A research execution and its writable workspace share this identifier.
/// A revision receives a new identifier; an agent session is a separate runtime value.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ResearchId(pub Uuid);

impl Default for ResearchId {
    fn default() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for ResearchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Working,
    InputRequired { question: String },
    Completed,
    Exhausted { reason: String },
    Failed { reason: String },
    Cancelled,
}

impl Status {
    pub fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Exhausted { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Source {
    pub id: String,
    pub url: String,
    pub title: String,
    /// Retrieved evidence, distinct from the worker's interpretation in Finding.
    pub excerpt: String,
    #[serde(default)]
    pub needs_refresh: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Finding {
    pub text: String,
    pub sources: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentKind {
    Investigate,
    Synthesize,
    Review,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct TraceContext {
    pub traceparent: Option<String>,
    pub tracestate: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Assignment {
    #[serde(default)]
    pub deadline_unix_seconds: Option<u64>,
    #[serde(default)]
    pub trace_context: TraceContext,
    pub research_id: ResearchId,
    pub number: u32,
    pub kind: AssignmentKind,
    pub objective: String,
    pub focus: String,
    pub strategy: Strategy,
    pub format: OutputFormat,
    pub context: String,
    pub remaining_seconds: u64,
    pub remaining_tool_calls: u32,
}

impl Assignment {
    pub fn time_remaining(&self) -> std::time::Duration {
        let allowance = std::time::Duration::from_secs(self.remaining_seconds);
        match self.deadline_unix_seconds {
            Some(deadline) => allowance.min(time_until(deadline)),
            None => allowance,
        }
    }
}

pub fn time_until(deadline: u64) -> std::time::Duration {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch");
    std::time::Duration::from_secs(deadline).saturating_sub(now)
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum NextAction {
    Investigate {
        question: String,
        strategy: Option<Strategy>,
    },
    Synthesize,
    Finish,
    AskUser {
        question: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, Default)]
pub struct Usage {
    pub tool_calls: Option<u32>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct AssignmentResult {
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub uncertainties: Vec<String>,
    #[serde(default)]
    pub outline: Vec<String>,
    pub draft: Option<String>,
    pub next: NextAction,
    #[serde(default)]
    pub usage: Usage,
}
