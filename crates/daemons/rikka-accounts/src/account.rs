use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccountKind {
    Standard,
    Administrator,
}

impl AccountKind {
    pub(crate) fn from_code(code: i32) -> AccountKind {
        match code {
            1 => AccountKind::Administrator,
            _ => AccountKind::Standard,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub kind: AccountKind,
    pub icon: Option<PathBuf>,
}
