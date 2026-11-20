use rikka_gecko_protocol::geckoview::prompt::{
    AlertPrompt, ButtonPrompt, ButtonResponse, ButtonType, CheckResponse, PromptCheck, PromptId,
    PromptResponse, TextPrompt, TextResponse,
};

use crate::wire::Answer;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Check {
    pub label: String,
    pub checked: bool,
}

impl Check {
    fn from_wire(check: PromptCheck) -> Option<Check> {
        check.has_check.then(|| Check {
            label: check.check_msg.unwrap_or_default(),
            checked: check.check_value.unwrap_or(false),
        })
    }
}

#[derive(Debug)]
pub struct Alert {
    pub id: PromptId,
    pub title: Option<String>,
    pub message: Option<String>,
    pub check: Option<Check>,
    answer: Answer,
}

impl Alert {
    pub(crate) fn from_wire(id: PromptId, prompt: AlertPrompt, answer: Answer) -> Alert {
        Alert {
            id,
            title: prompt.title,
            message: prompt.msg,
            check: Check::from_wire(prompt.check),
            answer,
        }
    }

    pub fn dismiss(self, checked: Option<bool>) {
        self.answer.with(&PromptResponse::Check(CheckResponse {
            check_value: checked,
        }));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    pub kind: ButtonType,
    pub label: ButtonLabel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ButtonLabel {
    Ok,
    Cancel,
    Yes,
    No,
    Custom(String),
}

#[derive(Debug)]
pub struct Confirm {
    pub id: PromptId,
    pub title: Option<String>,
    pub message: Option<String>,
    pub buttons: Vec<Button>,
    pub check: Option<Check>,
    answer: Answer,
}

impl Confirm {
    pub(crate) fn from_wire(id: PromptId, prompt: ButtonPrompt, answer: Answer) -> Confirm {
        use rikka_gecko_protocol::geckoview::prompt::ButtonTitle;
        let kinds = [
            ButtonType::Positive,
            ButtonType::Neutral,
            ButtonType::Negative,
        ];
        let buttons = kinds
            .into_iter()
            .zip(prompt.btn_title)
            .zip(prompt.btn_custom_title)
            .filter_map(|((kind, title), custom)| {
                let label = match title? {
                    ButtonTitle::Ok => ButtonLabel::Ok,
                    ButtonTitle::Cancel => ButtonLabel::Cancel,
                    ButtonTitle::Yes => ButtonLabel::Yes,
                    ButtonTitle::No => ButtonLabel::No,
                    ButtonTitle::Custom => ButtonLabel::Custom(custom.unwrap_or_default()),
                };
                Some(Button { kind, label })
            })
            .collect();
        Confirm {
            id,
            title: prompt.title,
            message: prompt.msg,
            buttons,
            check: Check::from_wire(prompt.check),
            answer,
        }
    }

    pub fn press(self, button: ButtonType, checked: Option<bool>) {
        self.answer.with(&PromptResponse::Button(ButtonResponse {
            button,
            check_value: checked,
        }));
    }

    pub fn dismiss(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct TextInput {
    pub id: PromptId,
    pub title: Option<String>,
    pub message: Option<String>,
    pub value: Option<String>,
    pub check: Option<Check>,
    answer: Answer,
}

impl TextInput {
    pub(crate) fn from_wire(id: PromptId, prompt: TextPrompt, answer: Answer) -> TextInput {
        TextInput {
            id,
            title: prompt.title,
            message: prompt.msg,
            value: prompt.value,
            check: Check::from_wire(prompt.check),
            answer,
        }
    }

    pub fn submit(self, text: impl Into<String>, checked: Option<bool>) {
        self.answer.with(&PromptResponse::Text(TextResponse {
            text: text.into(),
            check_value: checked,
        }));
    }

    pub fn dismiss(self) {
        drop(self);
    }
}
