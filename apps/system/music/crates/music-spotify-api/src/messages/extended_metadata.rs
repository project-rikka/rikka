#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct BatchedEntityRequest {
    #[prost(message, repeated, tag = "2")]
    pub(crate) entity_request: Vec<EntityRequest>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct EntityRequest {
    #[prost(string, tag = "1")]
    pub(crate) entity_uri: String,
    #[prost(message, repeated, tag = "2")]
    pub(crate) query: Vec<ExtensionQuery>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ExtensionQuery {
    #[prost(int32, tag = "1")]
    pub(crate) extension_kind: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct BatchedExtensionResponse {
    #[prost(message, repeated, tag = "2")]
    pub(crate) extended_metadata: Vec<EntityExtensionDataArray>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct EntityExtensionDataArray {
    #[prost(message, repeated, tag = "3")]
    pub(crate) extension_data: Vec<EntityExtensionData>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct EntityExtensionData {
    #[prost(string, tag = "2")]
    pub(crate) entity_uri: String,
    #[prost(message, optional, tag = "3")]
    pub(crate) extension_data: Option<Any>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Any {
    #[prost(string, tag = "1")]
    pub(crate) type_url: String,
    #[prost(bytes = "vec", tag = "2")]
    pub(crate) value: Vec<u8>,
}
