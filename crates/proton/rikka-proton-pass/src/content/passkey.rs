#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct PasskeyCreationData {
    #[prost(string, tag = "1")]
    pub os_name: String,
    #[prost(string, tag = "2")]
    pub os_version: String,
    #[prost(string, tag = "3")]
    pub device_name: String,
    #[prost(string, tag = "4")]
    pub app_version: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Passkey {
    #[prost(string, tag = "1")]
    pub key_id: String,
    #[prost(bytes = "vec", tag = "2")]
    pub content: Vec<u8>,
    #[prost(string, tag = "3")]
    pub domain: String,
    #[prost(string, tag = "4")]
    pub rp_id: String,
    #[prost(string, tag = "5")]
    pub rp_name: String,
    #[prost(string, tag = "6")]
    pub user_name: String,
    #[prost(string, tag = "7")]
    pub user_display_name: String,
    #[prost(bytes = "vec", tag = "8")]
    pub user_id: Vec<u8>,
    #[prost(uint32, tag = "9")]
    pub create_time: u32,
    #[prost(string, tag = "10")]
    pub note: String,
    #[prost(bytes = "vec", tag = "11")]
    pub credential_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "12")]
    pub user_handle: Vec<u8>,
    #[prost(message, optional, tag = "13")]
    pub creation_data: Option<PasskeyCreationData>,
}
