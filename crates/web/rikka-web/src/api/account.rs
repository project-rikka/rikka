use std::future::Future;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::channel::{Link, decode};
use crate::error::Result;

const PROFILE: &str = "identity.getProfileUserInfo";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub id: String,
}

impl Account {
    pub(crate) fn current(
        link: &Link,
    ) -> impl Future<Output = Result<Option<Account>>> + Send + use<> {
        let called = link.call(PROFILE, query());
        async move { Account::from_reply(called.await) }
    }

    pub(crate) fn current_then(
        link: &Link,
        then: impl FnOnce(Result<Option<Account>>) + Send + 'static,
    ) {
        link.call_then(PROFILE, query(), move |reply| {
            then(Account::from_reply(reply));
        });
    }

    fn from_reply(reply: Result<Value>) -> Result<Option<Account>> {
        let account: Account = decode(reply)?;
        Ok((!account.email.is_empty()).then_some(account))
    }
}

fn query() -> Vec<Value> {
    vec![json!({ "accountStatus": "ANY" })]
}
