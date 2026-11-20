#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct PageRequest {
    #[prost(string, tag = "1")]
    pub(crate) username: String,
    #[prost(string, tag = "2")]
    pub(crate) set: String,
    #[prost(string, tag = "3")]
    pub(crate) pagination_token: String,
    #[prost(int32, tag = "4")]
    pub(crate) limit: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct PageResponse {
    #[prost(message, repeated, tag = "1")]
    pub(crate) items: Vec<CollectionItem>,
    #[prost(string, tag = "2")]
    pub(crate) next_page_token: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct CollectionItem {
    #[prost(string, tag = "1")]
    pub(crate) uri: String,
    #[prost(int32, tag = "2")]
    pub(crate) added_at: i32,
    #[prost(bool, tag = "3")]
    pub(crate) is_removed: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct WriteRequest {
    #[prost(string, tag = "1")]
    pub(crate) username: String,
    #[prost(string, tag = "2")]
    pub(crate) set: String,
    #[prost(message, repeated, tag = "3")]
    pub(crate) items: Vec<CollectionItem>,
    #[prost(string, tag = "4")]
    pub(crate) client_update_id: String,
}
