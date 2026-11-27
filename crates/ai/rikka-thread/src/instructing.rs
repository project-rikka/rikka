use crate::assistant::Assistant;
use crate::revision;
use crate::thread::Thread;

#[derive(Debug)]
pub(crate) struct Instructing {
    pub(crate) sent: Vec<String>,
    pub(crate) restated: Option<String>,
    pub(crate) recorded: Option<Vec<String>>,
}

impl Instructing {
    pub(crate) fn new(assistant: &Assistant, thread: &Thread, model: &str) -> Instructing {
        let standing = assistant.standing(model);
        let Some((sent, conveyed)) = thread.instructed() else {
            return Instructing {
                sent: standing.clone(),
                restated: None,
                recorded: Some(standing),
            };
        };

        let sent = sent.instructions().unwrap_or_default().to_vec();
        let sections = revision::sections(conveyed.instructions().unwrap_or_default(), &standing);
        if sections.is_empty() {
            return Instructing {
                sent,
                restated: None,
                recorded: None,
            };
        }

        let restated = std::iter::once(assistant.revision_heading())
            .chain(sections.iter().map(String::as_str))
            .collect::<Vec<&str>>()
            .join("\n\n");
        Instructing {
            sent,
            restated: Some(restated),
            recorded: Some(standing),
        }
    }

    pub(crate) fn sent(assistant: &Assistant, thread: &Thread, model: &str) -> Vec<String> {
        match thread.instructed() {
            Some((sent, _)) => sent.instructions().unwrap_or_default().to_vec(),
            None => assistant.standing(model),
        }
    }
}
