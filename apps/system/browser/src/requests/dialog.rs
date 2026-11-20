use std::collections::VecDeque;

use rikka_web::{
    Address, Alert, AllowPopup, AllowRedirect, Check, Confirm, ConfirmFolderUpload, ConfirmLeave,
    ConfirmRepost, CreditCard, LoginEntry, PickColor, PickDateTime, PromptId, SelectAutofill,
    TextInput,
};

#[derive(Debug)]
pub(crate) enum Dialog {
    Alert(Alert),
    Confirm(Confirm),
    Text(TextInput),
    Leave(ConfirmLeave),
    Repost(ConfirmRepost),
    Popup(AllowPopup),
    Redirect(AllowRedirect),
    FolderUpload(ConfirmFolderUpload),
    Color(PickColor),
    DateTime(PickDateTime),
    Login(SelectAutofill<LoginEntry>),
    Card(SelectAutofill<CreditCard>),
    Address(SelectAutofill<Address>),
}

impl Dialog {
    pub(crate) fn id(&self) -> &PromptId {
        match self {
            Dialog::Alert(prompt) => &prompt.id,
            Dialog::Confirm(prompt) => &prompt.id,
            Dialog::Text(prompt) => &prompt.id,
            Dialog::Leave(prompt) => &prompt.id,
            Dialog::Repost(prompt) => &prompt.id,
            Dialog::Popup(prompt) => &prompt.id,
            Dialog::Redirect(prompt) => &prompt.id,
            Dialog::FolderUpload(prompt) => &prompt.id,
            Dialog::Color(prompt) => &prompt.id,
            Dialog::DateTime(prompt) => &prompt.id,
            Dialog::Login(prompt) => &prompt.id,
            Dialog::Card(prompt) => &prompt.id,
            Dialog::Address(prompt) => &prompt.id,
        }
    }

    pub(crate) fn check(&self) -> Option<&Check> {
        match self {
            Dialog::Alert(prompt) => prompt.check.as_ref(),
            Dialog::Confirm(prompt) => prompt.check.as_ref(),
            Dialog::Text(prompt) => prompt.check.as_ref(),
            _ => None,
        }
    }

    pub(crate) fn picks_for_field(&self) -> bool {
        matches!(
            self,
            Dialog::DateTime(_) | Dialog::Login(_) | Dialog::Card(_) | Dialog::Address(_)
        )
    }

    pub(crate) fn answer(self, accepted: bool, typed: String, checked: Option<bool>) {
        match self {
            Dialog::Alert(prompt) => prompt.dismiss(checked),
            Dialog::Confirm(prompt) => {
                let pressed = prompt
                    .buttons
                    .iter()
                    .map(|button| button.kind)
                    .find(|kind| (*kind == rikka_web::ButtonType::Positive) == accepted);
                match pressed {
                    Some(kind) => prompt.press(kind, checked),
                    None => prompt.dismiss(),
                }
            }
            Dialog::Text(prompt) if accepted => prompt.submit(typed, checked),
            Dialog::Text(prompt) => prompt.dismiss(),
            Dialog::Leave(prompt) => prompt.leave(accepted),
            Dialog::Repost(prompt) => prompt.allow(accepted),
            Dialog::Popup(prompt) => prompt.allow(accepted),
            Dialog::Redirect(prompt) => prompt.allow(accepted),
            Dialog::FolderUpload(prompt) => prompt.allow(accepted),
            Dialog::Color(prompt) if accepted && !typed.is_empty() => prompt.choose(typed),
            Dialog::Color(prompt) => prompt.dismiss(),
            Dialog::DateTime(prompt) if accepted && typed.is_empty() => prompt.clear(),
            Dialog::DateTime(prompt) if accepted => prompt.choose(typed),
            Dialog::DateTime(prompt) => prompt.dismiss(),
            Dialog::Login(prompt) => prompt.dismiss(),
            Dialog::Card(prompt) => prompt.dismiss(),
            Dialog::Address(prompt) => prompt.dismiss(),
        }
    }

    pub(crate) fn pick(self, index: usize) {
        match self {
            Dialog::Login(prompt) => pick(prompt, index),
            Dialog::Card(prompt) => pick(prompt, index),
            Dialog::Address(prompt) => pick(prompt, index),
            other => other.answer(false, String::new(), None),
        }
    }
}

fn pick<T: serde::Serialize>(mut prompt: SelectAutofill<T>, index: usize) {
    match index < prompt.options.len() {
        true => {
            let option = prompt.options.swap_remove(index);
            prompt.select(option);
        }
        false => prompt.dismiss(),
    }
}

#[derive(Debug, Default)]
pub(crate) struct Dialogs {
    queue: VecDeque<Dialog>,
}

impl Dialogs {
    pub(crate) fn push(&mut self, dialog: Dialog) -> bool {
        let idle = self.queue.is_empty();
        self.queue.push_back(dialog);
        idle
    }

    pub(crate) fn current(&self) -> Option<&Dialog> {
        self.queue.front()
    }

    pub(crate) fn take(&mut self) -> Option<Dialog> {
        self.queue.pop_front()
    }

    pub(crate) fn withdraw(&mut self, id: &PromptId) -> Option<Dialog> {
        let at = self.queue.iter().position(|dialog| dialog.id() == id)?;
        self.queue.remove(at)
    }
}
