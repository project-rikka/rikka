use rikka_web::{Prompt, PromptId, ShareResult};

use crate::requests::ChoiceRequest;
use crate::requests::dialog::Dialog;
use crate::shell::Shell;
use browser_model::TabId;

impl Shell {
    pub(crate) fn prompted(&mut self, tab: TabId, prompt: Prompt) {
        let dialog = match prompt {
            Prompt::Alert(prompt) => Dialog::Alert(prompt),
            Prompt::Confirm(prompt) => Dialog::Confirm(prompt),
            Prompt::Text(prompt) => Dialog::Text(prompt),
            Prompt::BeforeUnload(prompt) => Dialog::Leave(prompt),
            Prompt::Repost(prompt) => Dialog::Repost(prompt),
            Prompt::Popup(prompt) => Dialog::Popup(prompt),
            Prompt::Redirect(prompt) => Dialog::Redirect(prompt),
            Prompt::FolderUpload(prompt) => Dialog::FolderUpload(prompt),
            Prompt::Color(prompt) => Dialog::Color(prompt),
            Prompt::DateTime(prompt) => Dialog::DateTime(prompt),
            Prompt::SelectLogin(prompt) => Dialog::Login(prompt),
            Prompt::SelectCreditCard(prompt) => Dialog::Card(prompt),
            Prompt::SelectAddress(prompt) => Dialog::Address(prompt),
            Prompt::SignIn(prompt) => return self.sign_in_asked(tab, prompt),
            Prompt::Choose(prompt) => return self.choice_asked(tab, prompt),
            Prompt::Files(prompt) => return self.files_asked(tab, prompt),
            Prompt::Share(share) => {
                if let Some(uri) = share.uri.as_deref().or(share.text.as_deref()) {
                    rikka_ui::clipboard::set_text(uri);
                }
                share.finish(ShareResult::Success);
                return;
            }
            Prompt::SaveLogin(prompt) => return prompt.dismiss(),
            _ => return,
        };
        self.dialog_arrived(tab, dialog);
    }

    fn choice_asked(&mut self, tab: TabId, prompt: rikka_web::Choose) {
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        let opened = requests.next_opening();
        let chosen = chosen_ids(&prompt.choices);
        let replaced = requests.edit(tab).choosing.replace(ChoiceRequest {
            prompt,
            chosen,
            opened,
        });
        drop(replaced);
    }

    pub(crate) fn prompt_updated(&mut self, tab: TabId, prompt: Prompt) {
        let Prompt::Choose(updated) = prompt else {
            return;
        };
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        if let Some(choosing) = requests
            .edit_existing(tab)
            .and_then(|requests| requests.choosing.as_mut())
            .filter(|choosing| choosing.prompt.id == updated.id)
        {
            choosing.chosen = chosen_ids(&updated.choices);
            choosing.prompt.choices = updated.choices.clone();
        }
    }

    pub(crate) fn prompt_withdrawn(&mut self, tab: TabId, id: &PromptId) {
        self.dialog_withdrawn(tab, id);
        let page_services = self.browser().page_services();
        let mut requests = page_services.requests_mut();
        let Some(tab_requests) = requests.edit_existing(tab) else {
            return;
        };
        if tab_requests
            .choosing
            .as_ref()
            .is_some_and(|choosing| choosing.prompt.id == *id)
        {
            tab_requests.choosing = None;
        }
        if tab_requests
            .signing
            .as_ref()
            .is_some_and(|signing| signing.request.id == *id)
        {
            tab_requests.signing = None;
        }
    }
}

fn chosen_ids(choices: &[rikka_web::Choice]) -> Vec<String> {
    choices
        .iter()
        .flat_map(|choice| std::iter::once(choice).chain(choice.items.iter().flatten()))
        .filter(|choice| choice.selected)
        .map(|choice| choice.id.clone())
        .collect()
}
