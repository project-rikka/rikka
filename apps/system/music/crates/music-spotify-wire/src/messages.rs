#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ClientHello {
    #[prost(message, optional, tag = "10")]
    pub(crate) build_info: Option<BuildInfo>,
    #[prost(int32, repeated, packed = "false", tag = "30")]
    pub(crate) cryptosuites_supported: Vec<i32>,
    #[prost(message, optional, tag = "50")]
    pub(crate) login_crypto_hello: Option<LoginCryptoHelloUnion>,
    #[prost(bytes = "vec", required, tag = "60")]
    pub(crate) client_nonce: Vec<u8>,
    #[prost(bytes = "vec", optional, tag = "70")]
    pub(crate) padding: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct BuildInfo {
    #[prost(int32, required, tag = "10")]
    pub(crate) product: i32,
    #[prost(int32, repeated, packed = "false", tag = "20")]
    pub(crate) product_flags: Vec<i32>,
    #[prost(int32, required, tag = "30")]
    pub(crate) platform: i32,
    #[prost(uint64, required, tag = "40")]
    pub(crate) version: u64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginCryptoHelloUnion {
    #[prost(message, optional, tag = "10")]
    pub(crate) diffie_hellman: Option<LoginCryptoDiffieHellmanHello>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginCryptoDiffieHellmanHello {
    #[prost(bytes = "vec", required, tag = "10")]
    pub(crate) gc: Vec<u8>,
    #[prost(uint32, required, tag = "20")]
    pub(crate) server_keys_known: u32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ApResponseMessage {
    #[prost(message, optional, tag = "10")]
    pub(crate) challenge: Option<ApChallenge>,
    #[prost(message, optional, tag = "30")]
    pub(crate) login_failed: Option<ApLoginFailed>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ApChallenge {
    #[prost(message, optional, tag = "10")]
    pub(crate) login_crypto_challenge: Option<LoginCryptoChallengeUnion>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginCryptoChallengeUnion {
    #[prost(message, optional, tag = "10")]
    pub(crate) diffie_hellman: Option<LoginCryptoDiffieHellmanChallenge>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginCryptoDiffieHellmanChallenge {
    #[prost(bytes = "vec", optional, tag = "10")]
    pub(crate) gs: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "30")]
    pub(crate) gs_signature: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ApLoginFailed {
    #[prost(int32, optional, tag = "10")]
    pub(crate) error_code: Option<i32>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ClientResponsePlaintext {
    #[prost(message, optional, tag = "10")]
    pub(crate) login_crypto_response: Option<LoginCryptoResponseUnion>,
    #[prost(message, optional, tag = "20")]
    pub(crate) pow_response: Option<PowResponseUnion>,
    #[prost(message, optional, tag = "30")]
    pub(crate) crypto_response: Option<CryptoResponseUnion>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginCryptoResponseUnion {
    #[prost(message, optional, tag = "10")]
    pub(crate) diffie_hellman: Option<LoginCryptoDiffieHellmanResponse>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginCryptoDiffieHellmanResponse {
    #[prost(bytes = "vec", required, tag = "10")]
    pub(crate) hmac: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct PowResponseUnion {}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct CryptoResponseUnion {}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ClientResponseEncrypted {
    #[prost(message, optional, tag = "10")]
    pub(crate) login_credentials: Option<LoginCredentials>,
    #[prost(message, optional, tag = "50")]
    pub(crate) system_info: Option<SystemInfo>,
    #[prost(string, optional, tag = "70")]
    pub(crate) version_string: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginCredentials {
    #[prost(string, optional, tag = "10")]
    pub(crate) username: Option<String>,
    #[prost(int32, required, tag = "20")]
    pub(crate) typ: i32,
    #[prost(bytes = "vec", optional, tag = "30")]
    pub(crate) auth_data: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct SystemInfo {
    #[prost(int32, required, tag = "10")]
    pub(crate) cpu_family: i32,
    #[prost(int32, required, tag = "60")]
    pub(crate) os: i32,
    #[prost(string, optional, tag = "90")]
    pub(crate) system_information_string: Option<String>,
    #[prost(string, optional, tag = "100")]
    pub(crate) device_id: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ApWelcome {
    #[prost(string, optional, tag = "10")]
    pub(crate) canonical_username: Option<String>,
    #[prost(bytes = "vec", optional, tag = "40")]
    pub(crate) reusable_auth_credentials: Option<Vec<u8>>,
}
