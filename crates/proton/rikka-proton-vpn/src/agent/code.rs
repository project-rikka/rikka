use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AgentCode {
    Unknown,
    GuestSession,
    CertificateExpired,
    CertificateRevoked,
    KeyUsedMultipleTimes,
    RestrictedServer,
    BadCertificateSignature,
    CertificateNotProvided,
    MaxSessionsUnknown,
    MaxSessionsFree,
    MaxSessionsBasic,
    MaxSessionsPlus,
    MaxSessionsVisionary,
    MaxSessionsPro,
    ServerError,
    PolicyViolationLowPlan,
    PolicyViolationDelinquent,
    TorrentNotAllowed,
    BadBehaviour,
    Other(i64),
}

const CODES: [(i64, AgentCode); 19] = [
    (0, AgentCode::Unknown),
    (86100, AgentCode::GuestSession),
    (86101, AgentCode::CertificateExpired),
    (86102, AgentCode::CertificateRevoked),
    (86103, AgentCode::KeyUsedMultipleTimes),
    (86104, AgentCode::RestrictedServer),
    (86105, AgentCode::BadCertificateSignature),
    (86106, AgentCode::CertificateNotProvided),
    (86110, AgentCode::MaxSessionsUnknown),
    (86111, AgentCode::MaxSessionsFree),
    (86112, AgentCode::MaxSessionsBasic),
    (86113, AgentCode::MaxSessionsPlus),
    (86114, AgentCode::MaxSessionsVisionary),
    (86115, AgentCode::MaxSessionsPro),
    (86150, AgentCode::ServerError),
    (86151, AgentCode::PolicyViolationLowPlan),
    (86152, AgentCode::PolicyViolationDelinquent),
    (86153, AgentCode::TorrentNotAllowed),
    (86154, AgentCode::BadBehaviour),
];

impl AgentCode {
    #[must_use]
    pub fn from_code(code: i64) -> AgentCode {
        CODES
            .iter()
            .find(|(known, _)| *known == code)
            .map_or(AgentCode::Other(code), |(_, named)| *named)
    }

    #[must_use]
    pub fn code(self) -> i64 {
        match self {
            AgentCode::Other(code) => code,
            named => CODES
                .iter()
                .find(|(_, known)| *known == named)
                .map_or(0, |(code, _)| *code),
        }
    }

    #[must_use]
    pub fn is_max_sessions(self) -> bool {
        matches!(
            self,
            AgentCode::MaxSessionsUnknown
                | AgentCode::MaxSessionsFree
                | AgentCode::MaxSessionsBasic
                | AgentCode::MaxSessionsPlus
                | AgentCode::MaxSessionsVisionary
                | AgentCode::MaxSessionsPro
        )
    }
}

impl fmt::Display for AgentCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}
