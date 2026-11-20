use std::fmt;
use std::sync::Arc;

use rikka_llm::{Effort, ModelInfo};

use crate::account::Account;
use crate::account_id::AccountId;
use crate::connected_model::ConnectedModel;
use crate::error::{Error, Result};
use crate::selection::Selection;
use rikka_llm_providers::Renewal;

const NONE: &str = "none";

#[derive(Clone, Debug)]
pub struct ModelRoster {
    accounts: Arc<Vec<Account>>,
    selected: Option<AccountId>,
    effort: Effort,
    renewal: Renewal,
}

impl ModelRoster {
    pub(crate) fn new(
        accounts: Arc<Vec<Account>>,
        selected: Option<AccountId>,
        effort: Effort,
        renewal: Renewal,
    ) -> ModelRoster {
        ModelRoster {
            accounts,
            selected,
            effort,
            renewal,
        }
    }

    pub(crate) fn none() -> ModelRoster {
        ModelRoster {
            accounts: Arc::new(Vec::new()),
            selected: None,
            effort: Effort::default(),
            renewal: Renewal::none(),
        }
    }

    fn models(&self) -> impl Iterator<Item = (&Account, &ModelInfo)> {
        self.ready()
            .flat_map(|account| account.models().iter().map(move |model| (account, model)))
    }

    #[must_use]
    pub fn summary(&self) -> String {
        let models: Vec<String> = self
            .models()
            .map(|(account, model)| {
                format!(
                    "\"{}\" ({}{})",
                    model.name(),
                    account.label(),
                    Levels(model)
                )
            })
            .collect();
        match models.is_empty() {
            true => NONE.to_owned(),
            false => models.join(", "),
        }
    }

    pub fn connect(&self, selection: Option<&Selection>) -> Result<ConnectedModel> {
        let Some(selection) = selection else {
            let account = self.selected_account().ok_or(Error::Unconfigured)?;
            let model = account.connect(&self.renewal)?;
            return Ok(ConnectedModel::from_bench(
                self.clone(),
                model,
                account.model_info().cloned(),
                self.effort,
            ));
        };

        let account = self
            .account(selection.account())
            .ok_or(Error::Unconfigured)?;
        let model = account.connect_to(selection.model(), &self.renewal)?;
        let info = account.find_model(selection.model()).cloned();

        Ok(ConnectedModel::from_bench(
            self.clone(),
            model,
            info,
            selection.effort(),
        ))
    }

    pub fn connect_named(&self, named: &str, effort: Option<Effort>) -> Result<ConnectedModel> {
        let (account, info) = self.find(named).ok_or_else(|| Error::Unoffered {
            named: named.to_owned(),
            offered: self.summary(),
        })?;
        let model = account.connect_to(info.id(), &self.renewal)?;

        Ok(ConnectedModel::from_bench(
            self.clone(),
            model,
            Some(info.clone()),
            effort.unwrap_or(self.effort),
        ))
    }

    fn find(&self, named: &str) -> Option<(&Account, &ModelInfo)> {
        let named = named.trim();
        let plain = simplified(named);
        if plain.is_empty() {
            return None;
        }

        let found = |taken: fn(&ModelInfo) -> &str, exact: bool| {
            self.ordered().find_map(|account| {
                let model = account.models().iter().find(|model| match exact {
                    true => taken(model) == named,
                    false => simplified(taken(model)) == plain,
                })?;
                Some((account, model))
            })
        };

        found(ModelInfo::id, true)
            .or_else(|| found(ModelInfo::name, true))
            .or_else(|| found(ModelInfo::id, false))
            .or_else(|| found(ModelInfo::name, false))
    }

    fn selected_account(&self) -> Option<&Account> {
        self.selected.and_then(|id| self.account(id))
    }

    fn account(&self, id: AccountId) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id() == id)
    }

    fn ready(&self) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(|account| account.is_ready())
    }

    fn ordered(&self) -> impl Iterator<Item = &Account> {
        self.selected_account().into_iter().chain(
            self.ready()
                .filter(move |account| Some(account.id()) != self.selected),
        )
    }
}

struct Levels<'a>(&'a ModelInfo);

impl fmt::Display for Levels<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, effort) in self.0.efforts().iter().enumerate() {
            let separator = if index == 0 { "; " } else { "/" };
            write!(out, "{separator}{}", effort.id())?;
        }
        Ok(())
    }
}

fn simplified(named: &str) -> String {
    named
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|letter| letter.to_ascii_lowercase())
        .collect()
}
