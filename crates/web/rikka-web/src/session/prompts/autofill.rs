use rikka_gecko_protocol::geckoview::autocomplete::{
    LoginEntry, LoginSaveHint, SelectHint, SelectOption,
};
use rikka_gecko_protocol::geckoview::prompt::{
    LoginSavePrompt, PromptId, PromptResponse, Selection, SelectionResponse,
};
use serde::Serialize;

use crate::wire::Answer;

#[derive(Debug)]
pub struct SaveLogin {
    pub id: PromptId,
    pub hint: LoginSaveHint,
    pub logins: Vec<LoginEntry>,
    answer: Answer,
}

impl SaveLogin {
    pub(crate) fn from_wire(id: PromptId, prompt: LoginSavePrompt, answer: Answer) -> SaveLogin {
        SaveLogin {
            id,
            hint: prompt.hint,
            logins: prompt.logins,
            answer,
        }
    }

    pub fn save(self, login: LoginEntry) {
        self.answer.with(&PromptResponse::Login(SelectionResponse {
            selection: Selection {
                value: login,
                hint: SelectHint::NONE,
            },
        }));
    }

    pub fn dismiss(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct SelectAutofill<T> {
    pub id: PromptId,
    pub options: Vec<SelectOption<T>>,
    answer: Answer,
}

impl<T: Serialize> SelectAutofill<T> {
    pub(crate) fn from_wire(
        id: PromptId,
        options: Vec<SelectOption<T>>,
        answer: Answer,
    ) -> SelectAutofill<T> {
        SelectAutofill {
            id,
            options,
            answer,
        }
    }

    pub fn select(self, option: SelectOption<T>) {
        self.answer.with(&SelectionResponse {
            selection: Selection {
                value: option.value,
                hint: option.hint,
            },
        });
    }

    pub fn dismiss(self) {
        drop(self);
    }
}
