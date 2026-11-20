use paint_document::{Area, Document, FileFormat};
use rikka_image_edit::Profile;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DocumentInfo {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: FileFormat,
    pub(crate) profile: Option<Profile>,
    pub(crate) opaque: bool,
    pub(crate) bits: u8,
    pub(crate) mode: &'static str,
}

impl DocumentInfo {
    pub(crate) fn from_document(document: &Document) -> DocumentInfo {
        DocumentInfo {
            width: document.width(),
            height: document.height(),
            format: document.format(),
            profile: document.profile().cloned(),
            opaque: document.is_opaque(),
            bits: document.bits(),
            mode: document.color_mode(),
        }
    }

    pub(crate) fn canvas(&self) -> Area {
        Area::sized(self.width, self.height)
    }
}
