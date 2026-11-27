use rikka_types::Point;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenu {
    pub frame: i64,
    pub frame_url: Option<String>,
    pub at: Option<Point>,
    pub link: Option<ContextLink>,
    pub image: Option<ContextImage>,
    pub media: Option<ContextMedia>,
    pub selection: String,
    pub editable: Option<Editable>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ContextLink {
    pub url: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ContextImage {
    pub url: String,
    #[serde(default)]
    pub alt: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ContextMedia {
    pub url: String,
    pub kind: MediaKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Editable {
    pub input_type: Option<String>,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste(String),
    SelectAll,
    Delete,
}

impl EditCommand {
    pub(crate) fn exec(&self) -> (&'static str, Option<&str>) {
        match self {
            EditCommand::Undo => ("undo", None),
            EditCommand::Redo => ("redo", None),
            EditCommand::Cut => ("cut", None),
            EditCommand::Copy => ("copy", None),
            EditCommand::Paste(text) => ("insertText", Some(text)),
            EditCommand::SelectAll => ("selectAll", None),
            EditCommand::Delete => ("delete", None),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct ContextReport {
    pub(crate) link: Option<ContextLink>,
    pub(crate) image: Option<ContextImage>,
    pub(crate) media: Option<ContextMedia>,
    #[serde(default)]
    pub(crate) selection: String,
    pub(crate) editable: Option<Editable>,
}

impl ContextReport {
    pub(crate) fn menu(
        self,
        frame: i64,
        frame_url: Option<String>,
        at: Option<Point>,
    ) -> ContextMenu {
        ContextMenu {
            frame,
            frame_url,
            at,
            link: self.link,
            image: self.image,
            media: self.media,
            selection: self.selection,
            editable: self.editable,
        }
    }
}
