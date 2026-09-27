//! Mutable working material. Only the workflow parent applies assignment results.
use crate::research::*;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::DirBuilderExt;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Workspace {
    #[serde(default)]
    pub questions: BTreeMap<String, ResearchQuestion>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    pub id: ResearchId,
    pub owner: String,
    pub request: ResearchRequest,
    pub status: Status,
    pub sources: BTreeMap<String, Source>,
    pub notes: Vec<Finding>,
    pub uncertainties: Vec<String>,
    pub outline: Vec<String>,
    pub draft: String,
    pub assignments_completed: u32,
    pub tool_calls_observed: u32,
    pub tool_usage_complete: bool,
}

impl Workspace {
    pub fn new(owner: String, request: ResearchRequest) -> Result<Self, ResearchError> {
        request.validate()?;
        Ok(Self {
            questions: BTreeMap::new(),
            attachments: Vec::new(),
            id: ResearchId::default(),
            owner,
            request,
            status: Status::Working,
            sources: BTreeMap::new(),
            notes: Vec::new(),
            uncertainties: Vec::new(),
            outline: Vec::new(),
            draft: String::new(),
            assignments_completed: 0,
            tool_calls_observed: 0,
            tool_usage_complete: true,
        })
    }

    pub fn include_attachment_sources(&mut self) {
        for attachment in &mut self.attachments {
            if let Some(source) = self.sources.values().find(|s| s.url == attachment.uri) {
                attachment.source_id = source.id.clone();
                continue;
            }
            let mut number = 1_000_000;
            while self.sources.contains_key(&format!("S{number}")) {
                number += 1;
            }
            let id = format!("S{number}");
            attachment.source_id = id.clone();
            self.sources.insert(
                id.clone(),
                Source {
                    id,
                    url: attachment.uri.clone(),
                    title: attachment.name.clone(),
                    excerpt: attachment.excerpt.clone(),
                    needs_refresh: false,
                },
            );
        }
    }

    pub fn authorize(&self, principal: &str) -> Result<(), ResearchError> {
        if self.owner == principal {
            Ok(())
        } else {
            Err(ResearchError::Unavailable)
        }
    }

    pub fn revise(&self, principal: &str, request: ResearchRequest) -> Result<Self, ResearchError> {
        self.authorize(principal)?;
        if !self.status.terminal() {
            return Err(ResearchError::Invalid(
                "revise a terminal investigation; supply requested input to an active one".into(),
            ));
        }
        let mut revision = Self::new(principal.to_owned(), request)?;
        revision.attachments = self.attachments.clone();
        revision.sources = self.sources.clone();
        for source in revision.sources.values_mut() {
            source.needs_refresh = true;
        }
        revision.questions = self.questions.clone();
        revision.notes = self.notes.clone();
        revision.uncertainties = self.uncertainties.clone();
        revision.outline = self.outline.clone();
        revision.draft = self.draft.clone();
        Ok(revision)
    }

    /// Apply a complete worker return without accepting late results after termination.
    /// The workflow serializes calls; workers never persist this workspace directly.
    pub fn apply(&mut self, result: AssignmentResult) -> Result<NextAction, ResearchError> {
        if self.status != Status::Working {
            return Err(ResearchError::Invalid(
                "investigation is not accepting worker results".into(),
            ));
        }
        let mut sources = self.sources.clone();
        for source in result.sources {
            if source.excerpt.trim().is_empty() || source.id.trim().is_empty() {
                return Err(ResearchError::Invalid(
                    "source evidence needs an identifier and excerpt".into(),
                ));
            }
            if let Some(existing) = sources.get(&source.id)
                && existing.url != source.url
            {
                return Err(ResearchError::Invalid(
                    "source identifier already refers to another URL".into(),
                ));
            }
            sources.insert(source.id.clone(), source);
        }
        for finding in &result.findings {
            for id in &finding.sources {
                if !sources.contains_key(id) {
                    return Err(ResearchError::Invalid(format!(
                        "finding cites unavailable source {id}"
                    )));
                }
            }
        }
        if let Some(draft) = &result.draft {
            check_citations(draft, &sources)?;
        }
        if self.request.policy == ResearchPolicy::MultiAgent {
            for question in result.questions {
                self.questions.insert(question.id.clone(), question);
            }
        } else if self.request.policy.allows_direct_completion() {
            self.questions = result
                .questions
                .into_iter()
                .map(|q| (q.id.clone(), q))
                .collect();
        }
        self.sources = sources;
        self.notes.extend(result.findings);
        self.uncertainties = result.uncertainties;
        if !result.outline.is_empty() {
            self.outline = result.outline;
        }
        if let Some(draft) = result.draft {
            self.draft = draft;
        }
        self.assignments_completed += 1;
        match result.usage.tool_calls {
            Some(count) => {
                self.tool_calls_observed = self.tool_calls_observed.saturating_add(count)
            }
            None => self.tool_usage_complete = false,
        }
        Ok(result.next)
    }

    /// Select evidence relevant to the assignment, within a character budget.
    pub fn context(&self, focus: &str) -> String {
        let terms: Vec<_> = focus.split_whitespace().map(str::to_lowercase).collect();
        let mut evidence: Vec<_> = self.sources.values().collect();
        evidence.sort_by_key(|s| {
            let text = format!("{} {}", s.title, s.excerpt).to_lowercase();
            std::cmp::Reverse(
                terms
                    .iter()
                    .filter(|term| text.contains(term.as_str()))
                    .count(),
            )
        });
        let mut output = String::from(
            "Selected context: additional evidence, notes, or draft text may be omitted by the context budget.\n",
        );
        let limit = self.request.limits.context_chars;
        append_bounded(
            &mut output,
            &format!(
                "Research questions: {:?}\nOpen questions: {:?}\nOutline: {:?}\n",
                self.questions, self.uncertainties, self.outline
            ),
            limit / 4,
        );
        let evidence_limit = limit * 3 / 4;
        for source in evidence {
            append_bounded(
                &mut output,
                &format!(
                    "\nSOURCE [{}] {} {} {}\n{}\n",
                    source.id,
                    source.title,
                    source.url,
                    if source.needs_refresh {
                        "INHERITED: recheck time-sensitive facts"
                    } else {
                        ""
                    },
                    source.excerpt
                ),
                evidence_limit,
            );
        }
        for note in self.notes.iter().rev() {
            append_bounded(
                &mut output,
                &format!("\nINTERPRETATION: {} {:?}\n", note.text, note.sources),
                limit * 7 / 8,
            );
        }
        append_bounded(
            &mut output,
            &format!("\nCURRENT DRAFT:\n{}", self.draft),
            limit,
        );
        output
    }

    pub fn report(&self) -> String {
        let mut report = self.draft.clone();
        if report.is_empty() {
            report = format!("# Partial research\n\n{}\n", self.request.objective);
        }
        if self.draft.is_empty() || self.status != Status::Completed {
            if !self.draft.is_empty() {
                report.push_str("\n\n## Collected findings\n\nThe draft above may predate the latest evidence. These findings have not necessarily all been synthesized into it.\n");
            }
            for note in &self.notes {
                report.push_str(&format!(
                    "\n- {} {}",
                    note.text,
                    note.sources
                        .iter()
                        .map(|id| format!("[{id}]"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        }
        if self.draft.is_empty() || self.status != Status::Completed {
            for question in self
                .questions
                .values()
                .filter(|q| !q.answer.trim().is_empty())
            {
                report.push_str(&format!(
                    "\n\n### {}\n\n{} {}",
                    question.question,
                    question.answer,
                    question
                        .sources
                        .iter()
                        .map(|id| format!("[{id}]"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
                if !question.remaining_gap.is_empty() {
                    report.push_str(&format!(
                        "\n\nRemaining question: {}",
                        question.remaining_gap
                    ));
                }
            }
        }
        if !self.uncertainties.is_empty() {
            report.push_str("\n\n## Uncertainty and limitations\n");
            for question in &self.uncertainties {
                report.push_str(&format!("\n- {question}"));
            }
        }
        match &self.status {
            Status::Exhausted { reason } | Status::Failed { reason } => {
                report.push_str(&format!("\n\nResearch stopped: {reason}."))
            }
            Status::Cancelled => {
                report.push_str("\n\nResearch was cancelled; this is partial material.")
            }
            _ => (),
        }
        if self.sources.values().any(|source| source.needs_refresh) {
            report.push_str("\n\nSome evidence was inherited from an earlier investigation; time-sensitive facts require rechecking.");
        }
        report.push_str("\n\n## Sources\n");
        for source in self.sources.values() {
            report.push_str(&format!(
                "\n[{}]: {} \"{}\"\n",
                source.id,
                source.url,
                source.title.replace('"', "'")
            ));
        }
        report
    }
}

fn append_bounded(output: &mut String, text: &str, limit: usize) {
    let available = limit.saturating_sub(output.chars().count());
    if text.chars().count() <= available {
        output.push_str(text);
    } else {
        const MARKER: &str = "\n[truncated]\n";
        let marker_len = MARKER.chars().count();
        if available >= marker_len {
            output.extend(text.chars().take(available - marker_len));
            output.push_str(MARKER);
        }
    }
}

/// Citation syntax is [S<number>]. This checks existence, not semantic support.
pub fn check_citations(
    draft: &str,
    sources: &BTreeMap<String, Source>,
) -> Result<(), ResearchError> {
    for rest in draft.split("[S").skip(1) {
        if let Some((number, _)) = rest.split_once(']')
            && !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
            && !sources.contains_key(&format!("S{number}"))
        {
            return Err(ResearchError::Invalid(format!(
                "draft cites unavailable source S{number}"
            )));
        }
    }
    Ok(())
}

/// Local snapshots support CLI use. Restate keeps its own authoritative workflow state.
/// This store is single-writer per workspace and is not a second execution scheduler.
pub struct WorkspaceStore {
    root: PathBuf,
}

impl WorkspaceStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ResearchError> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(root.as_ref())?;
        Ok(Self {
            root: root.as_ref().to_owned(),
        })
    }
    pub fn save(&self, workspace: &Workspace) -> Result<(), ResearchError> {
        let destination = self.root.join(format!("{}.json", workspace.id));
        let temporary = self.root.join(format!("{}.tmp", workspace.id));
        std::fs::write(&temporary, serde_json::to_vec(workspace)?)?;
        std::fs::rename(temporary, destination)?;
        Ok(())
    }
    pub fn load(&self, id: ResearchId, principal: &str) -> Result<Workspace, ResearchError> {
        let bytes = std::fs::read(self.root.join(format!("{id}.json")))?;
        let workspace: Workspace = serde_json::from_slice(&bytes)?;
        workspace.authorize(principal)?;
        Ok(workspace)
    }
}
