use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CtapStatus(u8);

impl CtapStatus {
    pub const INVALID_COMMAND: CtapStatus = CtapStatus(0x01);
    pub const INVALID_PARAMETER: CtapStatus = CtapStatus(0x02);
    pub const INVALID_LENGTH: CtapStatus = CtapStatus(0x03);
    pub const CBOR_UNEXPECTED_TYPE: CtapStatus = CtapStatus(0x11);
    pub const INVALID_CBOR: CtapStatus = CtapStatus(0x12);
    pub const MISSING_PARAMETER: CtapStatus = CtapStatus(0x14);
    pub const LIMIT_EXCEEDED: CtapStatus = CtapStatus(0x15);
    pub const CREDENTIAL_EXCLUDED: CtapStatus = CtapStatus(0x19);
    pub const UNSUPPORTED_ALGORITHM: CtapStatus = CtapStatus(0x26);
    pub const OPERATION_DENIED: CtapStatus = CtapStatus(0x27);
    pub const KEY_STORE_FULL: CtapStatus = CtapStatus(0x28);
    pub const UNSUPPORTED_OPTION: CtapStatus = CtapStatus(0x2b);
    pub const INVALID_OPTION: CtapStatus = CtapStatus(0x2c);
    pub const KEEPALIVE_CANCEL: CtapStatus = CtapStatus(0x2d);
    pub const NO_CREDENTIALS: CtapStatus = CtapStatus(0x2e);
    pub const USER_ACTION_TIMEOUT: CtapStatus = CtapStatus(0x2f);
    pub const NOT_ALLOWED: CtapStatus = CtapStatus(0x30);
    pub const PIN_INVALID: CtapStatus = CtapStatus(0x31);
    pub const PIN_BLOCKED: CtapStatus = CtapStatus(0x32);
    pub const PIN_AUTH_INVALID: CtapStatus = CtapStatus(0x33);
    pub const PIN_AUTH_BLOCKED: CtapStatus = CtapStatus(0x34);
    pub const PIN_NOT_SET: CtapStatus = CtapStatus(0x35);
    pub const PUAT_REQUIRED: CtapStatus = CtapStatus(0x36);
    pub const PIN_POLICY_VIOLATION: CtapStatus = CtapStatus(0x37);
    pub const REQUEST_TOO_LARGE: CtapStatus = CtapStatus(0x39);
    pub const ACTION_TIMEOUT: CtapStatus = CtapStatus(0x3a);
    pub const UP_REQUIRED: CtapStatus = CtapStatus(0x3b);
    pub const UV_BLOCKED: CtapStatus = CtapStatus(0x3c);
    pub const UV_INVALID: CtapStatus = CtapStatus(0x3f);
    pub const UNAUTHORIZED_PERMISSION: CtapStatus = CtapStatus(0x40);
    pub const OTHER: CtapStatus = CtapStatus(0x7f);

    #[must_use]
    pub const fn new(code: u8) -> CtapStatus {
        CtapStatus(code)
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        self.0
    }

    fn name(self) -> Option<&'static str> {
        Some(match self {
            CtapStatus::INVALID_COMMAND => "CTAP1_ERR_INVALID_COMMAND",
            CtapStatus::INVALID_PARAMETER => "CTAP1_ERR_INVALID_PARAMETER",
            CtapStatus::INVALID_LENGTH => "CTAP1_ERR_INVALID_LENGTH",
            CtapStatus::CBOR_UNEXPECTED_TYPE => "CTAP2_ERR_CBOR_UNEXPECTED_TYPE",
            CtapStatus::INVALID_CBOR => "CTAP2_ERR_INVALID_CBOR",
            CtapStatus::MISSING_PARAMETER => "CTAP2_ERR_MISSING_PARAMETER",
            CtapStatus::LIMIT_EXCEEDED => "CTAP2_ERR_LIMIT_EXCEEDED",
            CtapStatus::CREDENTIAL_EXCLUDED => "CTAP2_ERR_CREDENTIAL_EXCLUDED",
            CtapStatus::UNSUPPORTED_ALGORITHM => "CTAP2_ERR_UNSUPPORTED_ALGORITHM",
            CtapStatus::OPERATION_DENIED => "CTAP2_ERR_OPERATION_DENIED",
            CtapStatus::KEY_STORE_FULL => "CTAP2_ERR_KEY_STORE_FULL",
            CtapStatus::UNSUPPORTED_OPTION => "CTAP2_ERR_UNSUPPORTED_OPTION",
            CtapStatus::INVALID_OPTION => "CTAP2_ERR_INVALID_OPTION",
            CtapStatus::KEEPALIVE_CANCEL => "CTAP2_ERR_KEEPALIVE_CANCEL",
            CtapStatus::NO_CREDENTIALS => "CTAP2_ERR_NO_CREDENTIALS",
            CtapStatus::USER_ACTION_TIMEOUT => "CTAP2_ERR_USER_ACTION_TIMEOUT",
            CtapStatus::NOT_ALLOWED => "CTAP2_ERR_NOT_ALLOWED",
            CtapStatus::PIN_INVALID => "CTAP2_ERR_PIN_INVALID",
            CtapStatus::PIN_BLOCKED => "CTAP2_ERR_PIN_BLOCKED",
            CtapStatus::PIN_AUTH_INVALID => "CTAP2_ERR_PIN_AUTH_INVALID",
            CtapStatus::PIN_AUTH_BLOCKED => "CTAP2_ERR_PIN_AUTH_BLOCKED",
            CtapStatus::PIN_NOT_SET => "CTAP2_ERR_PIN_NOT_SET",
            CtapStatus::PUAT_REQUIRED => "CTAP2_ERR_PUAT_REQUIRED",
            CtapStatus::PIN_POLICY_VIOLATION => "CTAP2_ERR_PIN_POLICY_VIOLATION",
            CtapStatus::REQUEST_TOO_LARGE => "CTAP2_ERR_REQUEST_TOO_LARGE",
            CtapStatus::ACTION_TIMEOUT => "CTAP2_ERR_ACTION_TIMEOUT",
            CtapStatus::UP_REQUIRED => "CTAP2_ERR_UP_REQUIRED",
            CtapStatus::UV_BLOCKED => "CTAP2_ERR_UV_BLOCKED",
            CtapStatus::UV_INVALID => "CTAP2_ERR_UV_INVALID",
            CtapStatus::UNAUTHORIZED_PERMISSION => "CTAP2_ERR_UNAUTHORIZED_PERMISSION",
            CtapStatus::OTHER => "CTAP1_ERR_OTHER",
            _ => return None,
        })
    }
}

impl fmt::Display for CtapStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(f, "{name} ({:#04x})", self.0),
            None => write!(f, "CTAP status {:#04x}", self.0),
        }
    }
}
