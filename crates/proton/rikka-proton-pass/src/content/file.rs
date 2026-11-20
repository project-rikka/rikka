use crate::content::non_empty;

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct FileMetadata {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub mime_type: String,
}

impl FileMetadata {
    pub fn mime(&self) -> Option<&str> {
        non_empty(&self.mime_type)
    }
}
