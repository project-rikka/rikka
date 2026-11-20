#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AllowedAndroidApp {
    #[prost(string, tag = "1")]
    pub package_name: String,
    #[prost(string, repeated, tag = "2")]
    pub hashes: Vec<String>,
    #[prost(string, tag = "3")]
    pub app_name: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AndroidSpecific {
    #[prost(message, repeated, tag = "1")]
    pub allowed_apps: Vec<AllowedAndroidApp>,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct PlatformSpecific {
    #[prost(message, optional, tag = "1")]
    pub android: Option<AndroidSpecific>,
}
