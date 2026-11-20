use std::collections::BTreeMap;

use rikka_llm::{Meter, Trouble, Window};

use crate::account::Account;
use crate::account_id::AccountId;
use crate::llm::Llm;
use crate::reading::Reading;

#[derive(Debug, Default)]
pub struct Readings(BTreeMap<AccountId, Reading>);

#[derive(Debug)]
pub struct UsageQuery {
    account: AccountId,
    nth: u64,
    meter: Meter,
}

#[derive(Debug)]
pub struct UsageReport {
    account: AccountId,
    nth: u64,
    read: Result<Vec<Window>, Trouble>,
}

impl Readings {
    #[must_use]
    pub fn get(&self, account: AccountId) -> Option<&Reading> {
        self.0.get(&account)
    }

    #[must_use]
    pub fn request(&mut self, llm: &Llm) -> Vec<UsageQuery> {
        let metered: Vec<AccountId> = llm.metered().map(Account::id).collect();
        let mut gauging = Vec::new();
        for account in metered {
            let nth = self.0.entry(account).or_default().next_request();
            match llm.meter(account) {
                Ok(meter) => gauging.push(UsageQuery {
                    account,
                    nth,
                    meter,
                }),
                Err(refused) => {
                    self.record(UsageReport {
                        account,
                        nth,
                        read: Err(Trouble::new(refused.to_string())),
                    });
                }
            }
        }
        gauging
    }

    pub fn record(&mut self, gauged: UsageReport) -> bool {
        self.0
            .get_mut(&gauged.account)
            .is_some_and(|reading| reading.record(gauged.nth, gauged.read))
    }

    pub fn retain(&mut self, mut keep: impl FnMut(AccountId) -> bool) {
        self.0.retain(|account, _| keep(*account));
    }
}

impl UsageQuery {
    pub async fn run(self) -> UsageReport {
        let read = self
            .meter
            .windows()
            .await
            .map_err(|refused| Trouble::from(&refused));
        UsageReport {
            account: self.account,
            nth: self.nth,
            read,
        }
    }
}
