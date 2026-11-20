#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct StorageResolveResponse {
    #[prost(int32, tag = "1")]
    pub(crate) result: i32,
    #[prost(string, repeated, tag = "2")]
    pub(crate) cdnurl: Vec<String>,
    #[prost(bytes = "vec", tag = "4")]
    pub(crate) fileid: Vec<u8>,
}
