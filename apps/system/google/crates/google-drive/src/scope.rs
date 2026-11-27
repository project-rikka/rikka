use crate::file::FOLDER;

pub const ROOT: &str = "root";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Folder(String),
    SharedWithMe,
    Recent,
    Starred,
    Trash,
    SharedDrives,
    Search(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Query {
    pub(crate) filter: String,
    pub(crate) order: Option<&'static str>,
}

impl Scope {
    #[must_use]
    pub fn my_drive() -> Scope {
        Scope::Folder(ROOT.to_owned())
    }

    pub(crate) fn query(&self) -> Query {
        let (filter, order) = match self {
            Scope::Folder(id) => (
                format!("'{}' in parents and trashed = false", quoted(id)),
                Some("folder,name_natural"),
            ),
            Scope::SharedWithMe => (
                "sharedWithMe = true and trashed = false".to_owned(),
                Some("sharedWithMeTime desc"),
            ),
            Scope::Recent => (
                format!("mimeType != '{FOLDER}' and trashed = false"),
                Some("recency desc"),
            ),
            Scope::Starred => (
                "starred = true and trashed = false".to_owned(),
                Some("folder,name_natural"),
            ),
            Scope::Trash => ("trashed = true".to_owned(), Some("modifiedTime desc")),
            Scope::SharedDrives => (String::new(), None),
            Scope::Search(text) => (
                format!(
                    "(name contains '{0}' or fullText contains '{0}') and trashed = false",
                    quoted(text.trim())
                ),
                None,
            ),
        };
        Query { filter, order }
    }

    pub(crate) fn cache_name(&self) -> Option<String> {
        let plain = |id: &str| {
            id.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        };
        match self {
            Scope::Folder(id) if !id.is_empty() && plain(id) => Some(format!("folder-{id}")),
            Scope::Folder(_) | Scope::Search(_) => None,
            Scope::SharedWithMe => Some("shared".to_owned()),
            Scope::Recent => Some("recent".to_owned()),
            Scope::Starred => Some("starred".to_owned()),
            Scope::Trash => Some("trash".to_owned()),
            Scope::SharedDrives => Some("drives".to_owned()),
        }
    }
}

fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\'', "\\'")
}
