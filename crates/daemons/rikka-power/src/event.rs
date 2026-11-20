use crate::action::{Operation, PowerCapabilities};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Event {
    Started {
        capabilities: PowerCapabilities,
        brightness: Option<f32>,
    },
    Brightness(Option<f32>),
    Answered {
        operation: Operation,
        error: Option<String>,
    },
}
