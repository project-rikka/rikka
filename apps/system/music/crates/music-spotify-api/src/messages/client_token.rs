#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ClientTokenRequest {
    #[prost(int32, tag = "1")]
    pub(crate) request_type: i32,
    #[prost(message, optional, tag = "2")]
    pub(crate) client_data: Option<ClientDataRequest>,
    #[prost(message, optional, tag = "3")]
    pub(crate) challenge_answers: Option<ChallengeAnswersRequest>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ClientDataRequest {
    #[prost(string, tag = "1")]
    pub(crate) client_version: String,
    #[prost(string, tag = "2")]
    pub(crate) client_id: String,
    #[prost(message, optional, tag = "3")]
    pub(crate) connectivity_sdk_data: Option<ConnectivitySdkData>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ConnectivitySdkData {
    #[prost(message, optional, tag = "1")]
    pub(crate) platform_specific_data: Option<PlatformSpecificData>,
    #[prost(string, tag = "2")]
    pub(crate) device_id: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct PlatformSpecificData {
    #[prost(message, optional, tag = "5")]
    pub(crate) desktop_linux: Option<NativeDesktopLinuxData>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct NativeDesktopLinuxData {
    #[prost(string, tag = "1")]
    pub(crate) system_name: String,
    #[prost(string, tag = "2")]
    pub(crate) system_release: String,
    #[prost(string, tag = "3")]
    pub(crate) system_version: String,
    #[prost(string, tag = "4")]
    pub(crate) hardware: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ChallengeAnswersRequest {
    #[prost(string, tag = "1")]
    pub(crate) state: String,
    #[prost(message, repeated, tag = "2")]
    pub(crate) answers: Vec<ChallengeAnswer>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ChallengeAnswer {
    #[prost(int32, tag = "1")]
    pub(crate) challenge_type: i32,
    #[prost(message, optional, tag = "4")]
    pub(crate) hash_cash: Option<HashCashAnswer>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct HashCashAnswer {
    #[prost(string, tag = "1")]
    pub(crate) suffix: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ClientTokenResponse {
    #[prost(int32, tag = "1")]
    pub(crate) response_type: i32,
    #[prost(message, optional, tag = "2")]
    pub(crate) granted_token: Option<GrantedTokenResponse>,
    #[prost(message, optional, tag = "3")]
    pub(crate) challenges: Option<ChallengesResponse>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct GrantedTokenResponse {
    #[prost(string, tag = "1")]
    pub(crate) token: String,
    #[prost(int32, tag = "2")]
    pub(crate) expires_after_seconds: i32,
    #[prost(int32, tag = "3")]
    pub(crate) refresh_after_seconds: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ChallengesResponse {
    #[prost(string, tag = "1")]
    pub(crate) state: String,
    #[prost(message, repeated, tag = "2")]
    pub(crate) challenges: Vec<Challenge>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Challenge {
    #[prost(int32, tag = "1")]
    pub(crate) challenge_type: i32,
    #[prost(message, optional, tag = "4")]
    pub(crate) evaluate_hashcash_parameters: Option<HashCashParameters>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct HashCashParameters {
    #[prost(int32, tag = "1")]
    pub(crate) length: i32,
    #[prost(string, tag = "2")]
    pub(crate) prefix: String,
}
