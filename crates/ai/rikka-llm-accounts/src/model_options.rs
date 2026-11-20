use rikka_llm::Effort;

use crate::account::Account;
use crate::account_id::AccountId;
use crate::llm::Llm;
use crate::selection::Selection;

#[derive(Clone, Debug)]
pub struct ModelOption {
    account: AccountId,
    account_label: String,
    id: String,
    name: String,
    efforts: Vec<Effort>,
    nearest: Option<Effort>,
}

impl ModelOption {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn account(&self) -> &str {
        &self.account_label
    }

    #[must_use]
    pub fn efforts(&self) -> &[Effort] {
        &self.efforts
    }

    #[must_use]
    pub fn nearest_effort(&self) -> Option<Effort> {
        self.nearest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Account(usize),
    Model(usize),
    Separator,
    Levels,
}

#[derive(Clone, Debug, Default)]
pub struct ModelOptions {
    picks: Vec<ModelOption>,
    rows: Vec<Row>,
    at: Option<usize>,
    effort: Effort,
}

impl ModelOptions {
    #[must_use]
    pub fn new(llm: &Llm, selection: Option<&Selection>) -> ModelOptions {
        let requested_effort = selection.map_or_else(|| llm.effort(), Selection::effort);
        let picks = llm
            .ready()
            .flat_map(|account| picks_for(account, requested_effort))
            .collect::<Vec<ModelOption>>();

        let selected = selection
            .map(|selection| (selection.account(), selection.model()))
            .or_else(|| {
                llm.selected_account()
                    .map(|account| (account.id(), account.model()))
            });
        let at = selected.and_then(|(account, id)| {
            picks
                .iter()
                .position(|pick| pick.account == account && pick.id == id)
        });

        ModelOptions::from_options(picks, at, requested_effort)
    }

    #[must_use]
    pub fn with_effort_row(mut self) -> ModelOptions {
        if self.has_levels() {
            self.rows.push(Row::Separator);
            self.rows.push(Row::Levels);
        }
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.picks.is_empty()
    }

    #[must_use]
    pub fn options(&self) -> &[ModelOption] {
        &self.picks
    }

    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.at
    }

    #[must_use]
    pub fn selected(&self) -> Option<&ModelOption> {
        self.picks.get(self.at?)
    }

    #[must_use]
    pub fn selection_at(&self, at: usize) -> Option<Selection> {
        let pick = self.picks.get(at)?;
        Some(Selection::new(
            pick.account,
            &pick.id,
            pick.nearest.unwrap_or(self.effort),
        ))
    }

    #[must_use]
    pub fn selection_for_row(&self, row: usize) -> Option<Selection> {
        match self.rows.get(row)? {
            Row::Model(at) => self.selection_at(*at),
            Row::Account(_) | Row::Separator | Row::Levels => None,
        }
    }

    #[must_use]
    pub fn efforts(&self) -> &[Effort] {
        self.selected().map_or(&[], ModelOption::efforts)
    }

    #[must_use]
    pub fn has_levels(&self) -> bool {
        !self.efforts().is_empty()
    }

    #[must_use]
    pub fn selected_effort_index(&self) -> Option<usize> {
        let nearest = self.selected()?.nearest?;
        self.efforts()
            .iter()
            .position(|effort_level| *effort_level == nearest)
    }

    #[must_use]
    pub fn selection_with_effort(&self, at: usize) -> Option<Selection> {
        let effort = *self.efforts().get(at)?;
        let pick = self.selected()?;
        Some(Selection::new(pick.account, &pick.id, effort))
    }

    fn from_options(picks: Vec<ModelOption>, at: Option<usize>, effort: Effort) -> ModelOptions {
        let rows = listing(&picks);
        ModelOptions {
            picks,
            rows,
            at,
            effort,
        }
    }
}

fn picks_for(account: &Account, effort: Effort) -> impl Iterator<Item = ModelOption> {
    let id = account.id();
    let label = account.label().to_owned();
    account.models().iter().map(move |model| ModelOption {
        account: id,
        account_label: label.clone(),
        id: model.id().to_owned(),
        name: model.name().to_owned(),
        efforts: model.efforts().to_vec(),
        nearest: model.nearest_effort(effort),
    })
}

fn listing(picks: &[ModelOption]) -> Vec<Row> {
    let first = picks.first().map(|pick| pick.account);
    let headed = picks.iter().any(|pick| Some(pick.account) != first);

    let mut rows = Vec::with_capacity(picks.len());
    let mut listed = None;
    for (at, pick) in picks.iter().enumerate() {
        if headed && listed != Some(pick.account) {
            rows.push(Row::Account(at));
        }
        listed = Some(pick.account);
        rows.push(Row::Model(at));
    }
    rows
}
