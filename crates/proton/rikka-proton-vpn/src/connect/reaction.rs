use crate::agent::{AgentCode, AgentError};
use crate::connect::state::Problem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reaction {
    Fail(Problem),
    RenewKey,
    RefreshCertificate,
    Wait,
}

impl Reaction {
    pub(crate) fn for_error(error: &AgentError) -> Reaction {
        match error.code {
            code if code.is_max_sessions() => Reaction::Fail(Problem::MaxSessions(code)),
            AgentCode::BadCertificateSignature | AgentCode::CertificateRevoked => {
                Reaction::RenewKey
            }
            AgentCode::CertificateExpired => Reaction::RefreshCertificate,
            AgentCode::KeyUsedMultipleTimes => Reaction::Fail(Problem::KeyUsedMultipleTimes),
            AgentCode::TorrentNotAllowed => Reaction::Fail(Problem::TorrentNotAllowed),
            AgentCode::BadBehaviour => Reaction::Fail(Problem::BadBehaviour),
            AgentCode::PolicyViolationLowPlan => Reaction::Fail(Problem::LowPlan),
            AgentCode::PolicyViolationDelinquent => Reaction::Fail(Problem::Delinquent),
            AgentCode::ServerError | AgentCode::Unknown => Reaction::Fail(Problem::ServerError),
            AgentCode::RestrictedServer => Reaction::Wait,
            code if error.is_final => Reaction::Fail(Problem::Agent {
                code,
                description: error.description.clone(),
            }),
            _ => Reaction::Wait,
        }
    }
}
