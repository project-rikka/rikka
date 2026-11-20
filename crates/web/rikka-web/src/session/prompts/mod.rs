mod autofill;
mod dialogs;
mod pickers;
mod targets;

use rikka_gecko_protocol::geckoview::autocomplete::{Address, CreditCard, LoginEntry};
use rikka_gecko_protocol::geckoview::prompt::{self as wire, PromptId, PromptKind};

use crate::wire::Answer;

pub use autofill::{SaveLogin, SelectAutofill};
pub use dialogs::{Alert, Button, ButtonLabel, Check, Confirm, TextInput};
pub use pickers::{Choose, ConfirmFolderUpload, PickColor, PickDateTime, PickFiles, SignIn};
pub use targets::{AllowPopup, AllowRedirect, ConfirmLeave, ConfirmRepost, Share};

#[derive(Debug)]
#[non_exhaustive]
pub enum Prompt {
    Alert(Alert),
    Confirm(Confirm),
    Text(TextInput),
    SignIn(SignIn),
    Choose(Choose),
    Color(PickColor),
    DateTime(PickDateTime),
    Files(PickFiles),
    FolderUpload(ConfirmFolderUpload),
    Popup(AllowPopup),
    Redirect(AllowRedirect),
    Repost(ConfirmRepost),
    BeforeUnload(ConfirmLeave),
    Share(Share),
    SaveLogin(SaveLogin),
    SelectLogin(SelectAutofill<LoginEntry>),
    SelectCreditCard(SelectAutofill<CreditCard>),
    SelectAddress(SelectAutofill<Address>),
}

impl Prompt {
    pub(crate) fn from_wire(prompt: wire::Prompt, answer: Answer) -> Prompt {
        let id = prompt.id;
        match prompt.kind {
            PromptKind::Alert(alert) => Prompt::Alert(Alert::from_wire(id, alert, answer)),
            PromptKind::Button(button) => Prompt::Confirm(Confirm::from_wire(id, button, answer)),
            PromptKind::Text(text) => Prompt::Text(TextInput::from_wire(id, text, answer)),
            PromptKind::Auth(auth) => Prompt::SignIn(SignIn::from_wire(id, auth, answer)),
            PromptKind::Choice(choice) => Prompt::Choose(Choose::from_wire(id, choice, answer)),
            PromptKind::Color(color) => Prompt::Color(PickColor::from_wire(id, color, answer)),
            PromptKind::DateTime(date) => {
                Prompt::DateTime(PickDateTime::from_wire(id, date, answer))
            }
            PromptKind::File(file) => Prompt::Files(PickFiles::from_wire(id, file, answer)),
            PromptKind::FolderUpload(folder) => {
                Prompt::FolderUpload(ConfirmFolderUpload::from_wire(id, folder, answer))
            }
            PromptKind::Popup(target) => Prompt::Popup(AllowPopup::from_wire(id, target, answer)),
            PromptKind::Redirect(target) => {
                Prompt::Redirect(AllowRedirect::from_wire(id, target, answer))
            }
            PromptKind::Repost {} => Prompt::Repost(ConfirmRepost::from_wire(id, answer)),
            PromptKind::BeforeUnload {} => {
                Prompt::BeforeUnload(ConfirmLeave::from_wire(id, answer))
            }
            PromptKind::Share(share) => Prompt::Share(Share::from_wire(id, share, answer)),
            PromptKind::LoginSave(save) => {
                Prompt::SaveLogin(SaveLogin::from_wire(id, save, answer))
            }
            PromptKind::LoginSelect(select) => {
                Prompt::SelectLogin(SelectAutofill::from_wire(id, select.options, answer))
            }
            PromptKind::CreditCardSelect(select) => {
                Prompt::SelectCreditCard(SelectAutofill::from_wire(id, select.options, answer))
            }
            PromptKind::AddressSelect(select) => {
                Prompt::SelectAddress(SelectAutofill::from_wire(id, select.options, answer))
            }
        }
    }

    #[must_use]
    pub fn id(&self) -> &PromptId {
        match self {
            Prompt::Alert(prompt) => &prompt.id,
            Prompt::Confirm(prompt) => &prompt.id,
            Prompt::Text(prompt) => &prompt.id,
            Prompt::SignIn(prompt) => &prompt.id,
            Prompt::Choose(prompt) => &prompt.id,
            Prompt::Color(prompt) => &prompt.id,
            Prompt::DateTime(prompt) => &prompt.id,
            Prompt::Files(prompt) => &prompt.id,
            Prompt::FolderUpload(prompt) => &prompt.id,
            Prompt::Popup(prompt) => &prompt.id,
            Prompt::Redirect(prompt) => &prompt.id,
            Prompt::Repost(prompt) => &prompt.id,
            Prompt::BeforeUnload(prompt) => &prompt.id,
            Prompt::Share(prompt) => &prompt.id,
            Prompt::SaveLogin(prompt) => &prompt.id,
            Prompt::SelectLogin(prompt) => &prompt.id,
            Prompt::SelectCreditCard(prompt) => &prompt.id,
            Prompt::SelectAddress(prompt) => &prompt.id,
        }
    }
}
