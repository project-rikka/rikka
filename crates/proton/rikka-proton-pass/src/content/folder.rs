#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Folder {
    #[prost(string, tag = "1")]
    pub name: String,
}
