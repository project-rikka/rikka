use std::fmt;

use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

macro_rules! response_codes {
    ($($variant:ident = $code:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum ResponseCode {
            $($variant,)+
            Unknown(i32),
        }

        impl ResponseCode {
            #[must_use]
            pub fn new(code: i32) -> ResponseCode {
                match code {
                    $($code => ResponseCode::$variant,)+
                    other => ResponseCode::Unknown(other),
                }
            }

            #[must_use]
            pub fn value(self) -> i32 {
                match self {
                    $(ResponseCode::$variant => $code,)+
                    ResponseCode::Unknown(code) => code,
                }
            }
        }
    };
}

response_codes! {
    Ok = 1000,
    MultipleResponses = 1001,
    Accepted = 1002,
    InvalidRequirements = 2000,
    InvalidValue = 2001,
    ValueOutOfBounds = 2003,
    NotNull = 2004,
    NotSameAsField = 2010,
    NotAllowed = 2011,
    Banned = 2028,
    FeatureDisabled = 2032,
    CurrencyFormat = 2053,
    NotExists = 2501,
    AppVersionBad = 5003,
    ApiVersionInvalid = 5005,
    AppVersionNotSupportedForExternalAccounts = 5099,
    BodyParseFailure = 6001,
    PasswordWrong = 8002,
    AuthSwitchToSso = 8100,
    AuthSwitchToSrp = 8101,
    HumanVerificationRequired = 9001,
    DeviceVerificationRequired = 9002,
    ScopeReauthLocked = 9101,
    ScopeReauthPassword = 9102,
    AccountFailedGeneric = 10_001,
    AccountDeleted = 10_002,
    AccountDisabled = 10_003,
    AccountCredentiallessInvalid = 10_200,
    AuthDeviceNotFound = 10_300,
    AuthDeviceNotActive = 10_301,
    AuthDeviceTokenInvalid = 10_302,
    AuthDeviceRejected = 10_303,
    UnprivatizationNotAllowed = 10_401,
    UserCreateNameInvalid = 12_081,
    UserCreateTokenInvalid = 12_087,
    UserExistsUsernameAlreadyUsed = 12_106,
    PaymentsSubscriptionNotExists = 22_110,
}

impl ResponseCode {
    #[must_use]
    pub fn is_ok(self) -> bool {
        matches!(
            self,
            ResponseCode::Ok | ResponseCode::MultipleResponses | ResponseCode::Accepted
        )
    }

    #[must_use]
    pub fn is_force_update(self) -> bool {
        matches!(
            self,
            ResponseCode::AppVersionBad | ResponseCode::ApiVersionInvalid
        )
    }

    #[must_use]
    pub fn is_missing_scope(self) -> bool {
        matches!(
            self,
            ResponseCode::ScopeReauthLocked | ResponseCode::ScopeReauthPassword
        )
    }
}

impl From<i32> for ResponseCode {
    fn from(code: i32) -> ResponseCode {
        ResponseCode::new(code)
    }
}

impl From<ResponseCode> for i32 {
    fn from(code: ResponseCode) -> i32 {
        code.value()
    }
}

impl fmt::Display for ResponseCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}

impl Serialize for ResponseCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.value())
    }
}

impl<'de> Deserialize<'de> for ResponseCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ResponseCode, D::Error> {
        i32::deserialize(deserializer).map(ResponseCode::new)
    }
}
