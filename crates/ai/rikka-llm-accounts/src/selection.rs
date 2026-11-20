use rikka_llm::{Effort, Model, ModelInfo};

use crate::account_id::AccountId;
use crate::error::Result;
use crate::llm::Llm;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Selection {
    #[serde(rename = "backend")]
    account: AccountId,
    model: String,
    effort: Effort,
}

impl Selection {
    #[must_use]
    pub fn new(
        account: impl Into<AccountId>,
        model: impl Into<String>,
        effort: Effort,
    ) -> Selection {
        Selection {
            account: account.into(),
            model: model.into(),
            effort,
        }
    }

    #[must_use]
    pub fn account(&self) -> AccountId {
        self.account
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub fn effort(&self) -> Effort {
        self.effort
    }

    #[must_use]
    pub fn model_info<'a>(&self, llm: &'a Llm) -> Option<&'a ModelInfo> {
        llm.account(self.account)?.find_model(&self.model)
    }

    pub fn connect(&self, llm: &Llm) -> Result<Model> {
        llm.connect_to(self.account, &self.model)
    }
}
