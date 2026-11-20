use crate::cbor::Value;

const NONE_FORMAT: &str = "none";
const PACKED_FORMAT: &str = "packed";
const CERTIFICATES: &str = "x5c";
const ATTESTED: u8 = 0x40;
const FLAGS_AT: usize = 32;
const AAGUID_AT: usize = 37;
const AAGUID_LENGTH: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttestationConveyance {
    #[default]
    None,
    Indirect,
    Direct,
    Enterprise,
}

impl AttestationConveyance {
    #[must_use]
    pub fn from_name(name: &str) -> Option<AttestationConveyance> {
        match name {
            "none" => Some(AttestationConveyance::None),
            "indirect" => Some(AttestationConveyance::Indirect),
            "direct" => Some(AttestationConveyance::Direct),
            "enterprise" => Some(AttestationConveyance::Enterprise),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            AttestationConveyance::None => "none",
            AttestationConveyance::Indirect => "indirect",
            AttestationConveyance::Direct => "direct",
            AttestationConveyance::Enterprise => "enterprise",
        }
    }
}

pub(crate) struct Attestation {
    pub(crate) format: String,
    pub(crate) statement: Value,
    pub(crate) auth_data: Vec<u8>,
}

impl Attestation {
    pub(crate) fn with_conveyance(mut self, conveyance: AttestationConveyance) -> Attestation {
        if conveyance != AttestationConveyance::None || self.is_self_attestation() {
            return self;
        }

        self.format = NONE_FORMAT.to_owned();
        self.statement = Value::Map(Vec::new());
        if let Some(aaguid) = self.aaguid_mut() {
            aaguid.fill(0);
        }
        self
    }

    pub(crate) fn object(&self) -> Vec<u8> {
        Value::map([
            ("fmt", Value::from(self.format.as_str())),
            ("attStmt", self.statement.clone()),
            ("authData", Value::bytes(self.auth_data.clone())),
        ])
        .encode()
    }

    fn is_self_attestation(&self) -> bool {
        self.format == PACKED_FORMAT
            && self.statement.get(CERTIFICATES).is_none()
            && self
                .aaguid()
                .is_some_and(|aaguid| aaguid.iter().all(|byte| *byte == 0))
    }

    fn aaguid(&self) -> Option<&[u8]> {
        let attested = self.auth_data.get(FLAGS_AT)? & ATTESTED != 0;
        attested
            .then(|| self.auth_data.get(AAGUID_AT..AAGUID_AT + AAGUID_LENGTH))
            .flatten()
    }

    fn aaguid_mut(&mut self) -> Option<&mut [u8]> {
        let attested = self.auth_data.get(FLAGS_AT)? & ATTESTED != 0;
        attested
            .then(|| self.auth_data.get_mut(AAGUID_AT..AAGUID_AT + AAGUID_LENGTH))
            .flatten()
    }
}
