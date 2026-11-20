use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ContentBlock {
    Text {
        #[serde(default)]
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Audio {
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        resource: ResourceContents,
    },
    ResourceLink {
        uri: String,
        #[serde(default)]
        name: String,
        description: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceContents {
    pub(crate) uri: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) blob: Option<String>,
}
