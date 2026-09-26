//! Research decisions are proposed by the worker; ordinary code bounds execution.
use crate::{research::*, workspace::Workspace};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Controller {
    pub workspace: Workspace,
    pub kind: AssignmentKind,
    pub focus: String,
    pub strategy: Strategy,
    pub started_at: u64,
    reviews: u32,
}

impl Controller {
    pub fn new(workspace: Workspace, now: u64) -> Self {
        Self {
            focus: workspace.request.objective.clone(),
            strategy: workspace.request.strategy,
            workspace,
            kind: AssignmentKind::Investigate,
            started_at: now,
            reviews: 0,
        }
    }

    pub fn assignment(&mut self, now: u64) -> Option<Assignment> {
        if self.workspace.status != Status::Working {
            return None;
        }
        let limits = &self.workspace.request.limits;
        let remaining_seconds = limits
            .wall_seconds
            .saturating_sub(now.saturating_sub(self.started_at));
        let remaining_tool_calls = limits
            .max_tool_calls
            .saturating_sub(self.workspace.tool_calls_observed);
        let remaining_assignments = limits
            .max_assignments
            .saturating_sub(self.workspace.assignments_completed);
        let reason = if remaining_seconds == 0 {
            Some("wall-clock limit")
        } else if remaining_assignments == 0 {
            Some("assignment limit")
        } else if remaining_tool_calls == 0 && self.kind == AssignmentKind::Investigate {
            Some("source-tool limit")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.workspace.status = Status::Exhausted {
                reason: reason.into(),
            };
            return None;
        }
        if remaining_assignments == 1 && self.kind == AssignmentKind::Investigate {
            self.kind = AssignmentKind::Synthesize;
            self.focus = "Use available evidence to produce the best partial answer; disclose unfinished research.".into();
        }
        Some(Assignment {
            research_id: self.workspace.id,
            number: self.workspace.assignments_completed + 1,
            kind: self.kind,
            objective: format!(
                "{}\n\nUser context:\n{}\n\nSource constraints:\n{}",
                self.workspace.request.objective,
                self.workspace.request.context,
                self.workspace.request.source_constraints.join("\n")
            ),
            focus: self.focus.clone(),
            strategy: self.strategy,
            format: self.workspace.request.format,
            context: self.workspace.context(&self.focus),
            remaining_seconds,
            remaining_tool_calls,
        })
    }

    pub fn complete(&mut self, result: AssignmentResult) -> Result<(), ResearchError> {
        let next = self.workspace.apply(result)?;
        if self.kind == AssignmentKind::Review {
            self.reviews += 1;
        }
        match next {
            NextAction::AskUser { question } => {
                self.workspace.status = Status::InputRequired { question };
            }
            NextAction::Investigate { question, strategy } if self.reviews < 2 => {
                self.focus = question;
                if let Some(strategy) = strategy {
                    self.strategy = strategy;
                }
                self.kind = AssignmentKind::Investigate;
            }
            NextAction::Synthesize
                if self.kind != AssignmentKind::Synthesize && self.reviews < 2 =>
            {
                self.kind = AssignmentKind::Synthesize;
                self.focus = "Write an answer suited to the user's purpose using selected evidence. Preserve uncertainty and cite consequential claims.".into();
            }
            _ => {
                match self.kind {
                    AssignmentKind::Investigate => {
                        self.kind = AssignmentKind::Synthesize;
                        self.focus = "Synthesize the findings into a cited answer for the requested purpose.".into();
                    }
                    AssignmentKind::Synthesize => {
                        if self.workspace.draft.trim().is_empty() {
                            return Err(ResearchError::Invalid("writer returned no answer".into()));
                        }
                        self.kind = AssignmentKind::Review;
                        self.focus = "Check question coverage, consequential citation support, serious alternatives and uncertainty. Request only a specific follow-up that could change the answer; otherwise finish.".into();
                    }
                    AssignmentKind::Review => {
                        if self.reviews >= 2 && !matches!(next, NextAction::Finish) {
                            self.workspace.uncertainties.push(
                                "Review requested further work after the bounded review allowance."
                                    .into(),
                            );
                        }
                        self.workspace.status = Status::Completed;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn provide_input(&mut self, input: &str) -> Result<(), ResearchError> {
        let Status::InputRequired { question } = &self.workspace.status else {
            return Err(ResearchError::Invalid(
                "investigation is not waiting for input".into(),
            ));
        };
        if input.trim().is_empty() || input.chars().count() > 16_000 {
            return Err(ResearchError::Invalid(
                "input must contain 1–16000 characters".into(),
            ));
        }
        let context = format!(
            "{}\nClarification question: {question}\nUser clarification: {input}",
            self.workspace.request.context
        );
        if context.chars().count() > 16_000 {
            return Err(ResearchError::Invalid(
                "combined user context and clarifications must fit within 16000 characters".into(),
            ));
        }
        self.workspace.request.context = context;
        self.workspace.status = Status::Working;
        Ok(())
    }
}
