use std::sync::Arc;

use rikka_llm::{Content, Message};

type Restating = Arc<dyn Fn() -> Option<String> + Send + Sync>;

const FOCUS: &str = include_str!("../prompts/focus.md");

const FOLDED: &str = include_str!("../prompts/folded.md");

const RESUMED: &str = include_str!("../prompts/resumed.md");

#[derive(Clone)]
pub struct Compaction {
    window: u64,
    output: u64,
    keep: usize,
    folding: CompactionPrompts,
    restating: Option<Restating>,
}

#[derive(Clone, Debug, Default)]
pub struct CompactionPrompts {
    instruction: String,
    framing: String,
    focusing: String,
    resuming: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trigger<'a> {
    Automatic,
    Overflowed,
    Asked(&'a str),
}

impl Trigger<'_> {
    pub(crate) fn shares_request(self) -> bool {
        self != Trigger::Overflowed
    }
}

impl CompactionPrompts {
    #[must_use]
    pub fn new(instruction: impl Into<String>) -> CompactionPrompts {
        CompactionPrompts {
            instruction: instruction.into(),
            framing: FOLDED.trim().to_owned(),
            focusing: FOCUS.trim().to_owned(),
            resuming: RESUMED.trim().to_owned(),
        }
    }

    #[must_use]
    pub fn framing(mut self, framing: impl Into<String>) -> Self {
        self.framing = framing.into();
        self
    }

    #[must_use]
    pub fn focus_heading(mut self, heading: impl Into<String>) -> Self {
        self.focusing = heading.into();
        self
    }

    #[must_use]
    pub fn resume_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.resuming = prompt.into();
        self
    }
}

impl Compaction {
    pub const KEEP: usize = 4;

    const RESERVE: u64 = 20_000;

    const MARGIN: u64 = 13_000;

    const WARNING: u64 = 20_000;

    #[must_use]
    pub fn new(window: u64, folding: CompactionPrompts) -> Compaction {
        Compaction {
            window,
            output: 0,
            keep: Compaction::KEEP,
            folding,
            restating: None,
        }
    }

    #[must_use]
    pub fn output(mut self, output: u64) -> Self {
        self.output = output;
        self
    }

    #[must_use]
    pub fn keep(mut self, keep: usize) -> Self {
        self.keep = keep;
        self
    }

    #[must_use]
    pub fn restatement(
        mut self,
        restating: impl Fn() -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        self.restating = Some(Arc::new(restating));
        self
    }

    fn folds_at(window: u64, output: u64) -> u64 {
        let usable = window.saturating_sub(reserve(output));
        usable.saturating_sub(Compaction::MARGIN).max(usable / 2)
    }

    #[must_use]
    pub fn is_near_limit(filled: u64, window: u64, output: u64) -> bool {
        let folds_at = Compaction::folds_at(window, output);
        folds_at > 0 && filled.saturating_add(Compaction::WARNING) >= folds_at
    }

    pub(crate) fn is_full(&self, tokens: u64) -> bool {
        self.window > 0 && tokens >= Compaction::folds_at(self.window, self.output)
    }

    pub(crate) fn summary_tokens(&self) -> u32 {
        u32::try_from(reserve(self.output)).unwrap_or(u32::MAX)
    }

    pub(crate) fn cut(&self, messages: &[Message]) -> usize {
        let mut cut = messages.len().saturating_sub(self.keep);
        while cut > 0 && messages.get(cut).is_some_and(is_tool_result) {
            cut -= 1;
        }
        match cut >= self.keep {
            true => cut,
            false => 0,
        }
    }

    pub(crate) fn instruction(&self, trigger: Trigger<'_>) -> String {
        let instruction = &self.folding.instruction;
        match trigger {
            Trigger::Asked(focus) if !focus.trim().is_empty() => {
                let focusing = &self.folding.focusing;
                match focusing.is_empty() {
                    true => format!("{instruction}\n\n{}", focus.trim()),
                    false => format!("{instruction}\n\n{focusing}\n\n{}", focus.trim()),
                }
            }
            _ => instruction.clone(),
        }
    }

    pub(crate) fn summary_message(
        &self,
        summary: &str,
        recalled: Option<String>,
        unloaded: Option<String>,
    ) -> String {
        joined([
            Some(self.folding.framing.clone()),
            Some(summary.to_owned()),
            recalled,
            unloaded,
        ])
    }

    pub(crate) fn live(&self, summary: &str, trigger: Trigger<'_>) -> String {
        let restated = self.restating.as_ref().and_then(|restating| restating());
        let resuming = match trigger {
            Trigger::Automatic | Trigger::Overflowed => Some(self.folding.resuming.clone()),
            Trigger::Asked(_) => None,
        };
        joined([Some(summary.to_owned()), restated, resuming])
    }
}

impl std::fmt::Debug for Compaction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Compaction")
            .field("window", &self.window)
            .field("output", &self.output)
            .field("keep", &self.keep)
            .field("folding", &self.folding)
            .field("restating", &self.restating.is_some())
            .finish()
    }
}

fn joined<const N: usize>(parts: [Option<String>; N]) -> String {
    parts
        .into_iter()
        .flatten()
        .map(|part| part.trim().to_owned())
        .filter(|part| !part.is_empty())
        .collect::<Vec<String>>()
        .join("\n\n")
}

fn reserve(output: u64) -> u64 {
    match output {
        0 => Compaction::RESERVE,
        output => output.min(Compaction::RESERVE),
    }
}

pub(crate) fn is_tool_result(message: &Message) -> bool {
    message
        .content()
        .iter()
        .any(|content| matches!(content, Content::ToolResult(_)))
}
