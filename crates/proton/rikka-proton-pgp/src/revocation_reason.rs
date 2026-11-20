use proton_rpgp::pgp::packet::RevocationCode;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RevocationReason {
    #[default]
    Unspecified,
    Superseded,
    Compromised,
    Retired,
}

impl RevocationReason {
    pub(crate) fn code(self) -> RevocationCode {
        match self {
            RevocationReason::Unspecified => RevocationCode::NoReason,
            RevocationReason::Superseded => RevocationCode::KeySuperseded,
            RevocationReason::Compromised => RevocationCode::KeyCompromised,
            RevocationReason::Retired => RevocationCode::KeyRetired,
        }
    }
}
