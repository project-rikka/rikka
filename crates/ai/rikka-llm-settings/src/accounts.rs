use std::collections::{BTreeMap, BTreeSet};

use crate::prelude::*;
use rikka_llm_accounts::{Account, AccountId, Backend, SignIn, Step};
use rikka_widgets::{column, row};

use crate::card::{self, Card};
use crate::endpoint_form::{self, EndpointForm, Filled};
use crate::llm_settings::LlmSettings;
use crate::models::{self, Models};
use crate::rows::muted;
use crate::signing::Signing;
use crate::usage::{self, Usage};

#[derive(Debug)]
struct SignInError {
    backend: Backend,
    reason: String,
}

#[derive(Debug)]
pub(crate) struct Accounts {
    keys: BTreeMap<Backend, TextDocument>,
    reveal: Option<Backend>,
    signing: Option<Signing>,
    pub(super) models: Models,
    pub(super) form: Option<EndpointForm>,
    started: u64,
    error: Option<SignInError>,
    added: BTreeSet<Backend>,
    adding: bool,
}

impl Accounts {
    pub(crate) fn new(llm: &rikka_llm_accounts::Llm) -> Accounts {
        Accounts {
            keys: fields(llm),
            reveal: None,
            signing: None,
            models: Models::new(),
            form: None,
            started: 0,
            error: None,
            added: BTreeSet::new(),
            adding: false,
        }
    }

    pub(crate) fn reread(&mut self, llm: &rikka_llm_accounts::Llm) {
        self.keys = fields(llm);
    }

    pub(crate) fn error(&self) -> Option<Label> {
        let error = self.error.as_ref()?;
        Some(tr!(
            "llm-settings.failed",
            provider = error.backend.label(),
            reason = error.reason.as_str()
        ))
    }

    pub(crate) fn reveal(&mut self, backend: Option<Backend>) {
        self.reveal = backend;
    }

    pub(crate) fn begin(&mut self, backend: Backend) {
        if let Some(running) = &self.signing {
            running.stop();
        }
        self.error = None;
        self.started += 1;
        self.signing = Some(Signing::start(backend, self.started));
    }

    pub(crate) fn stop(&self) {
        if let Some(signing) = &self.signing {
            signing.stop();
        }
    }

    pub(crate) fn adding(&mut self, adding: bool) {
        self.adding = adding;
    }

    pub(crate) fn include(&mut self, index: usize, llm: &rikka_llm_accounts::Llm) {
        match self.offered(llm).get(index).copied() {
            Some(backend) => {
                self.added.insert(backend);
            }
            None => self.form = Some(EndpointForm::new()),
        }
        self.adding = false;
    }

    pub(crate) fn edit_endpoint(&mut self, id: AccountId, llm: &rikka_llm_accounts::Llm) {
        if let Some(account) = llm.account(id) {
            self.form = Some(EndpointForm::editing(account));
        }
    }

    pub(crate) fn close_form(&mut self) {
        self.form = None;
    }

    pub(crate) fn save_form(&mut self, llm: &mut rikka_llm_accounts::Llm) {
        let Some(form) = self.form.as_mut() else {
            return;
        };
        let Some(Filled {
            name,
            endpoint,
            credentials,
        }) = form.filled()
        else {
            return;
        };
        let editing = form.edits();
        self.form = None;
        match editing {
            Some(id) => {
                llm.edit_endpoint(id, &name, endpoint, credentials);
                self.models.forget(id);
            }
            None => {
                let id = llm.add_endpoint(&name, endpoint, credentials);
                if let Some(account) = llm.account(id) {
                    self.models.open(account);
                }
            }
        }
    }

    pub(crate) fn remove_endpoint(&mut self, id: AccountId, llm: &mut rikka_llm_accounts::Llm) {
        if self
            .form
            .as_ref()
            .is_some_and(|form| form.edits() == Some(id))
        {
            self.form = None;
        }
        self.models.forget(id);
        llm.remove_endpoint(id);
    }

    pub(crate) fn sign_out(&mut self, backend: Backend, llm: &mut rikka_llm_accounts::Llm) {
        self.error = None;
        self.added.remove(&backend);
        llm.clear_credential(backend);
        self.commit_backend(backend, llm);
    }

    pub(crate) fn sign_in_step(&mut self, nth: u64, step: Step, llm: &mut rikka_llm_accounts::Llm) {
        if self.signing.as_ref().is_none_or(|signing| !signing.is(nth)) {
            return;
        }
        match step {
            Step::Prompt(_, prompt) => {
                if let Some(signing) = self.signing.as_mut() {
                    signing.prompted(prompt);
                }
            }
            Step::Done(backend, credential) => {
                self.signing = None;
                llm.set_credential(backend, credential);
                self.commit_backend(backend, llm);
            }
            Step::Failed(backend, reason) => {
                self.signing = None;
                self.error = Some(SignInError { backend, reason });
            }
        }
    }

    pub(crate) fn commit(&self, backend: Backend, llm: &mut rikka_llm_accounts::Llm) {
        if let Some(field) = self.keys.get(&backend) {
            llm.set_api_key(backend.into(), &field.text());
        }
    }

    pub(crate) fn commit_all(&self, llm: &mut rikka_llm_accounts::Llm) {
        for (backend, field) in &self.keys {
            llm.set_api_key((*backend).into(), &field.text());
        }
    }

    fn shown<'a>(&self, llm: &'a rikka_llm_accounts::Llm) -> Vec<&'a Account> {
        llm.accounts()
            .iter()
            .filter(|account| self.lists(account))
            .collect()
    }

    fn offered(&self, llm: &rikka_llm_accounts::Llm) -> Vec<Backend> {
        llm.accounts()
            .iter()
            .filter(|account| !self.lists(account))
            .filter_map(Account::backend)
            .collect()
    }

    fn lists(&self, account: &Account) -> bool {
        match account.backend() {
            Some(backend) => account.is_ready() || self.added.contains(&backend),
            None => true,
        }
    }

    fn commit_backend(&self, backend: Backend, llm: &rikka_llm_accounts::Llm) {
        if let (Some(field), Some(account)) = (self.keys.get(&backend), llm.account(backend.into()))
        {
            field.reset(account.stored_key());
        }
    }
}

pub(crate) fn view(accounts: &Accounts, llm: &rikka_llm_accounts::Llm, usage: &Usage) -> impl View {
    let accounts_shown = accounts.shown(llm);
    section(
        tr!("llm-settings.accounts"),
        column! {
            for (nth, account) in accounts_shown.iter().enumerate() {
                if nth > 0 {
                    rule();
                }
                account_card(accounts, account, usage, llm);
            }
            if accounts_shown.is_empty() && !accounts.adding && accounts.form.is_none() {
                muted(tr!("llm-settings.none"));
            }
            if let Some(form) = accounts.form.as_ref().filter(|form| form.edits().is_none()) {
                if !accounts_shown.is_empty() {
                    rule();
                }
                endpoint_form::view(form);
            }
            footer(accounts, llm);
        }
        .gap(Space::ROOMY)
        .fill_width(),
    )
}

fn account_card(
    accounts: &Accounts,
    account: &Account,
    usage: &Usage,
    llm: &rikka_llm_accounts::Llm,
) -> Element {
    if let Some(form) = accounts
        .form
        .as_ref()
        .filter(|form| form.edits() == Some(account.id()))
    {
        return endpoint_form::view(form).build();
    }
    let backend = account.backend();
    card::view(
        account,
        Card {
            typed: backend.and_then(|backend| accounts.keys.get(&backend)),
            revealed: backend.is_some() && accounts.reveal == backend,
            in_use: llm.selected_account().map(Account::id) == Some(account.id()),
            signing: accounts.signing.as_ref(),
            models: account
                .has_custom_models()
                .then(|| models::view(&accounts.models, account).build()),
            usage: usage::view(usage, account),
        },
    )
}

fn footer(accounts: &Accounts, llm: &rikka_llm_accounts::Llm) -> Option<Element> {
    if accounts.form.is_some() {
        return None;
    }
    if !accounts.adding {
        return Some(
            button(
                row! {
                    icon(MaterialSymbol::ADD).extent(Length::px(16.0));
                    text(tr!("llm-settings.add")).caption();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .secondary()
            .small()
            .on_press(|settings: &mut LlmSettings| settings.accounts.adding(true))
            .build(),
        );
    }

    let offered = accounts.offered(llm);
    let choices = offered
        .iter()
        .map(|backend| list_item(backend.label()).secondary(kind(*backend)))
        .chain(std::iter::once(
            list_item(tr!("llm-settings.endpoint.label"))
                .secondary(tr!("llm-settings.kind.endpoint")),
        ));
    Some(
        column! {
            list(choices)
                .selected(None)
                .control_size(ControlSize::Small)
                .on_select(|settings: &mut LlmSettings, index: usize| settings.accounts.include(index, &settings.llm))
                .fill_width();
            row! {
                spacer().grow(1.0);
                button(tr!("llm-settings.cancel"))
                    .ghost()
                    .small()
                    .on_press(|settings: &mut LlmSettings| settings.accounts.adding(false));
            }
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width()
        .build(),
    )
}

fn kind(backend: Backend) -> Label {
    match backend.sign_in() {
        SignIn::Key { .. } => tr!("llm-settings.kind.key"),
        SignIn::KeyOrBrowser { .. } => tr!("llm-settings.kind.either"),
        SignIn::Browser | SignIn::Code => tr!("llm-settings.kind.subscription"),
    }
}

fn fields(llm: &rikka_llm_accounts::Llm) -> BTreeMap<Backend, TextDocument> {
    Backend::all()
        .filter(|backend| !backend.is_subscription())
        .map(|backend| {
            let key = llm
                .account(backend.into())
                .map(Account::stored_key)
                .unwrap_or_default();
            (backend, TextDocument::new(key))
        })
        .collect()
}
