use rikka_compositor_shell::ToplevelId;
use slotmap::{Key, KeyData};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ToplevelKey(u64);

impl ToplevelKey {
    pub(crate) fn of(toplevel: ToplevelId) -> ToplevelKey {
        ToplevelKey(toplevel.data().as_ffi())
    }

    pub(crate) fn toplevel(self) -> ToplevelId {
        ToplevelId::from(KeyData::from_ffi(self.0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Claim {
    AppId(String),
    Title(String),
    Toplevel(ToplevelKey),
}

impl Claim {
    #[must_use]
    pub fn app_id(app_id: impl Into<String>) -> Claim {
        Claim::AppId(app_id.into())
    }

    #[must_use]
    pub fn title(token: impl Into<String>) -> Claim {
        Claim::Title(token.into())
    }

    #[must_use]
    pub fn toplevel(toplevel: ToplevelKey) -> Claim {
        Claim::Toplevel(toplevel)
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Claim::AppId(app_id) => app_id.clone(),
            Claim::Title(token) => format!("title-{token}"),
            Claim::Toplevel(key) => format!("toplevel-{}", key.0),
        }
    }
}
