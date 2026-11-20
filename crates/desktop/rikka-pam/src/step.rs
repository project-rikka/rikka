#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Prompt(Prompt),
    Message(Message),
    Finished(Outcome),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub serial: u64,
    pub text: String,
    pub echo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: MessageKind,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Granted,
    Denied(String),
    Cancelled,
    Error(String),
}

impl Outcome {
    #[must_use]
    pub fn is_granted(&self) -> bool {
        matches!(self, Outcome::Granted)
    }
}
