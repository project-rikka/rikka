#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginRequest {
    #[prost(message, optional, tag = "1")]
    pub(crate) client_info: Option<ClientInfo>,
    #[prost(bytes = "vec", tag = "2")]
    pub(crate) login_context: Vec<u8>,
    #[prost(message, optional, tag = "3")]
    pub(crate) challenge_solutions: Option<ChallengeSolutions>,
    #[prost(message, optional, tag = "100")]
    pub(crate) stored_credential: Option<StoredCredential>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ClientInfo {
    #[prost(string, tag = "1")]
    pub(crate) client_id: String,
    #[prost(string, tag = "2")]
    pub(crate) device_id: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct StoredCredential {
    #[prost(string, tag = "1")]
    pub(crate) username: String,
    #[prost(bytes = "vec", tag = "2")]
    pub(crate) data: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ChallengeSolutions {
    #[prost(message, repeated, tag = "1")]
    pub(crate) solutions: Vec<ChallengeSolution>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ChallengeSolution {
    #[prost(message, optional, tag = "1")]
    pub(crate) hashcash: Option<HashcashSolution>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct HashcashSolution {
    #[prost(bytes = "vec", tag = "1")]
    pub(crate) suffix: Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub(crate) duration: Option<ProtoDuration>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ProtoDuration {
    #[prost(int64, tag = "1")]
    pub(crate) seconds: i64,
    #[prost(int32, tag = "2")]
    pub(crate) nanos: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginResponse {
    #[prost(message, optional, tag = "1")]
    pub(crate) ok: Option<LoginOk>,
    #[prost(int32, optional, tag = "2")]
    pub(crate) error: Option<i32>,
    #[prost(message, optional, tag = "3")]
    pub(crate) challenges: Option<Challenges>,
    #[prost(bytes = "vec", tag = "5")]
    pub(crate) login_context: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LoginOk {
    #[prost(string, tag = "1")]
    pub(crate) username: String,
    #[prost(string, tag = "2")]
    pub(crate) access_token: String,
    #[prost(int32, tag = "4")]
    pub(crate) access_token_expires_in: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Challenges {
    #[prost(message, repeated, tag = "1")]
    pub(crate) challenges: Vec<Challenge>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Challenge {
    #[prost(message, optional, tag = "1")]
    pub(crate) hashcash: Option<HashcashChallenge>,
    #[prost(message, optional, tag = "2")]
    pub(crate) code: Option<CodeChallenge>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct HashcashChallenge {
    #[prost(bytes = "vec", tag = "1")]
    pub(crate) prefix: Vec<u8>,
    #[prost(int32, tag = "2")]
    pub(crate) length: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct CodeChallenge {}
