//! Question-driven research, with isolated initial investigations and evidence-led revision.
use crate::{research::*, workspace::Workspace};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Inquiry {
    plan: Option<ResearchPlan>,
    initial_context: String,
}

impl Inquiry {
    pub fn context(&self, workspace: &Workspace, kind: AssignmentKind, focus: &str) -> String {
        if matches!(
            kind,
            AssignmentKind::PrimaryResearch | AssignmentKind::IndependentResearch
        ) {
            return self.initial_context.clone();
        }
        // Review must see the complete answer, not a suffix left over after evidence packing.
        // Original passages remain available through the source-material tools.
        let mut context = format!(
            "CURRENT DRAFT:\n{}\n\nRESEARCH QUESTIONS:\n{}\n\nFINDINGS:\n{}\n\nUNRESOLVED:\n{}\n",
            workspace.draft,
            serde_json::to_string_pretty(&workspace.questions).expect("questions serialize"),
            serde_json::to_string_pretty(&workspace.notes).expect("findings serialize"),
            workspace.uncertainties.join("\n"),
        );
        let mut evidence = workspace.clone();
        evidence.draft.clear();
        evidence.questions.clear();
        evidence.notes.clear();
        evidence.uncertainties.clear();
        evidence.outline.clear();
        evidence.request.limits.context_chars = workspace
            .request
            .limits
            .context_chars
            .saturating_sub(context.chars().count());
        context.push_str(&evidence.context(focus));
        context
    }

    pub fn complete(
        &mut self,
        workspace: &mut Workspace,
        kind: &mut AssignmentKind,
        focus: &mut String,
        mut result: AssignmentResult,
    ) -> Result<(), ResearchError> {
        if let NextAction::AskUser { question } = &result.next {
            let question = question.clone();
            workspace.apply(result)?;
            workspace.status = Status::InputRequired { question };
            return Ok(());
        }
        if *kind == AssignmentKind::Reconnaissance {
            let plan = result.research_plan.as_ref().ok_or_else(|| {
                ResearchError::Invalid(
                    "reconnaissance returned no complementary research assignments".into(),
                )
            })?;
            if plan.primary.trim().is_empty()
                || plan.independent.trim().is_empty()
                || result.questions.is_empty()
            {
                return Err(ResearchError::Invalid(
                    "reconnaissance needs research questions and complementary assignments".into(),
                ));
            }
        }
        if *kind == AssignmentKind::Synthesize
            && result
                .draft
                .as_ref()
                .is_none_or(|draft| draft.trim().is_empty())
        {
            return Err(ResearchError::Invalid(
                "synthesis returned no answer".into(),
            ));
        }
        if matches!(
            kind,
            AssignmentKind::PrimaryResearch | AssignmentKind::IndependentResearch
        ) {
            for question in &mut result.questions {
                if let Some(previous) = workspace.questions.get(&question.id) {
                    if !previous.answer.is_empty() && previous.answer != question.answer {
                        question.answer = format!(
                            "Earlier finding: {}\nIndependent finding: {}",
                            previous.answer, question.answer
                        );
                    }
                    for source in &previous.sources {
                        if !question.sources.contains(source) {
                            question.sources.push(source.clone());
                        }
                    }
                    if !previous.remaining_gap.is_empty()
                        && previous.remaining_gap != question.remaining_gap
                    {
                        question.remaining_gap =
                            format!("{}\n{}", previous.remaining_gap, question.remaining_gap);
                    }
                }
            }
        }
        let plan = result.research_plan.clone();
        // Research and coverage assessment update evidence; only the writer/reviewer
        // can replace the complete answer to the original user request.
        if !matches!(kind, AssignmentKind::Synthesize | AssignmentKind::Review) {
            result.draft = None;
        }
        let next = workspace.apply(result)?;
        match *kind {
            AssignmentKind::Reconnaissance => {
                self.plan = plan;
                self.initial_context =
                    self.context(workspace, AssignmentKind::Reconnaissance, focus);
                *kind = AssignmentKind::PrimaryResearch;
                *focus = self.plan.as_ref().expect("checked plan").primary.clone();
            }
            AssignmentKind::PrimaryResearch => {
                *kind = AssignmentKind::IndependentResearch;
                *focus = self
                    .plan
                    .as_ref()
                    .expect("reconnaissance plan")
                    .independent
                    .clone();
            }
            AssignmentKind::IndependentResearch => {
                *kind = AssignmentKind::CoverageReview;
                *focus = "Combine the independent findings. Update questions and identify consequential unanswered needs, contradictions, and relevant discoveries outside the initial framing. Request a specific further investigation if needed; otherwise synthesize.".into();
            }
            AssignmentKind::Synthesize => {
                *kind = AssignmentKind::Review;
                *focus = "Independently check the complete draft against the user need and original sources. Verify consequential claims, qualifications and citations; identify missing important answers, including findings lost in synthesis. Retrieve evidence directly or request a specific follow-up. Finish only when remaining uncertainty is accurately disclosed.".into();
            }
            AssignmentKind::Review if matches!(next, NextAction::Finish) => {
                if workspace.draft.trim().is_empty() {
                    return Err(ResearchError::Invalid(
                        "review has no answer to approve".into(),
                    ));
                }
                workspace.status = Status::Completed;
            }
            AssignmentKind::CoverageReview | AssignmentKind::Review => match next {
                NextAction::Investigate { question, .. } => {
                    *kind = AssignmentKind::Investigate;
                    *focus = question;
                }
                _ => self.synthesize(kind, focus),
            },
            AssignmentKind::Investigate => {
                if let NextAction::Investigate { question, .. } = next {
                    *focus = question;
                } else {
                    self.synthesize(kind, focus);
                }
            }
            AssignmentKind::CompleteResearch => {
                return Err(ResearchError::Invalid(
                    "multi-agent research cannot complete in one assignment".into(),
                ));
            }
        }
        Ok(())
    }

    fn synthesize(&self, kind: &mut AssignmentKind, focus: &mut String) {
        *kind = AssignmentKind::Synthesize;
        *focus = "Answer the original user request using the research questions and supporting passages. Cover important questions before adding repetitive depth. Preserve significant unexpected findings, conflicting evidence and qualifications. Read full source material or retrieve missing evidence as needed. Return the complete cited answer; a separate reviewer follows.".into();
    }
}

pub fn guidance(kind: AssignmentKind) -> &'static str {
    match kind {
        AssignmentKind::Reconnaissance => {
            "\nYour responsibility is initial source reconnaissance and research design, not the final answer. Use a modest initial search to understand terminology and discover consequential angles. Return a compact question list and research_plan with two complementary, self-contained assignments (primary and independent). Explain relevant question IDs and why each investigation matters. Perspectives must be relevant to this domain, not invented disagreement. Return draft null; independent researchers follow."
        }
        AssignmentKind::PrimaryResearch | AssignmentKind::IndependentResearch => {
            "\nInvestigate the assigned questions in your own context. Use sources, develop evidence-backed answers in questions, and preserve significant unexpected findings with their evidence. Each question retains its original ID; give new questions distinct descriptive IDs. Preserve supporting passages and enough context to avoid misleading compression. Return draft null and research_plan null. Your findings go to a lead that combines independent investigations."
        }
        AssignmentKind::CoverageReview => {
            "\nYou have both initial investigations. Reconcile their answers without erasing disagreement, preserve new relevant questions and decide which important gaps merit further research. Do not claim adequate support merely because several URLs repeat one source. Return an explicit next action and findings explaining the assessment; draft null, research_plan null."
        }
        AssignmentKind::Review => {
            "\nUse original passages to check the draft, not just another agent's summary. Return findings describing the material checks and corrections. Ask for further investigation when a consequential evidence gap remains; do not approve merely because citations exist. An uncertainty can remain if clearly explained and further research is unlikely to resolve it. Return research_plan null."
        }
        _ => {
            "\nReturn research_plan null outside initial reconnaissance. Preserve supported details when revising."
        }
    }
}

pub fn render_findings(result: &AssignmentResult) -> String {
    let mut text = String::from("# Research handoff\n\n");
    if let Some(plan) = &result.research_plan {
        text.push_str(&format!(
            "Primary assignment: {}\n\nIndependent assignment: {}\n\n",
            plan.primary, plan.independent
        ));
    }
    for question in &result.questions {
        text.push_str(&format!(
            "## {}: {}\n\n{}\n\nSources: {}\n\nRemaining gap: {}\n\n",
            question.id,
            question.question,
            question.answer,
            question.sources.join(", "),
            question.remaining_gap
        ));
    }
    for finding in &result.findings {
        text.push_str(&format!(
            "- {} ({})\n",
            finding.text,
            finding.sources.join(", ")
        ));
    }
    for source in &result.sources {
        text.push_str(&format!(
            "\n## Source {}\n\n{}\n\nRetrieved excerpt:\n\n{}\n",
            source.id, source.url, source.excerpt
        ));
    }
    text.push_str(&format!(
        "\n## Remaining uncertainty\n\n{}\n\n## Next action\n\n{:?}\n",
        result.uncertainties.join("\n"),
        result.next
    ));
    if let Some(draft) = &result.draft {
        text.push_str(&format!("\n## Draft\n\n{draft}\n"));
    }
    text
}
