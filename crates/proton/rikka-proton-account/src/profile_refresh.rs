use crate::account_keys::AccountKeys;

#[derive(Debug)]
pub enum ProfileRefresh {
    Unchanged,
    Updated(Box<AccountKeys>),
    Rekeyed(Box<AccountKeys>),
}
