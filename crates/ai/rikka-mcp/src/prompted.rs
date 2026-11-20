use crate::part::Part;
use crate::prompt::worded;
use crate::protocol::GetPromptResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompted {
    description: Option<String>,
    parts: Vec<Part>,
}

impl Prompted {
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[must_use]
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    #[must_use]
    pub fn into_parts(self) -> Vec<Part> {
        self.parts
    }

    pub(crate) fn from_result(result: GetPromptResult) -> Prompted {
        Prompted {
            description: worded(result.description),
            parts: result
                .messages
                .into_iter()
                .map(|message| Part::from_block(message.content))
                .collect(),
        }
    }
}
