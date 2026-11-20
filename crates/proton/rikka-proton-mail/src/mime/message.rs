use crate::mime::content::MimeContent;
use crate::mime::headers::Headers;
use crate::mime::parser::{self, DefaultContentType};
use crate::mime::part::MimePart;

#[derive(Debug, Clone)]
pub struct MimeMessage {
    root: MimePart,
}

impl MimeMessage {
    #[must_use]
    pub fn parse(raw: impl AsRef<[u8]>) -> MimeMessage {
        MimeMessage {
            root: parser::parse(raw.as_ref(), DefaultContentType::Text, 0),
        }
    }

    #[must_use]
    pub fn root(&self) -> &MimePart {
        &self.root
    }

    #[must_use]
    pub fn headers(&self) -> &Headers {
        self.root.headers()
    }

    #[must_use]
    pub fn content(&self) -> MimeContent {
        MimeContent::from_part(&self.root)
    }

    pub(crate) fn root_mut(&mut self) -> &mut MimePart {
        &mut self.root
    }
}
