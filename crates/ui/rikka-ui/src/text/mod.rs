mod blink;
mod display_text;
mod document;
pub mod elide;
mod keying;
mod run;
pub mod selection;
mod shadow;
pub(crate) mod shaping;
mod style;
mod widget;

pub use blink::Blink;
pub use display_text::{BULLET, DisplayRevision, DisplayText};
pub use document::TextDocument;
pub use keying::KeyEdit;
pub use rikka_i18n::Label;
pub use rikka_text::{TextLayout, WordRule};
pub use rikka_text_document::{
    Chunks, Content, Endpoints, Erase, Granularity, LineEnding, Place, Rope, SelectionOrigin,
    Splice, Travel, place_of,
};
pub use run::TextRun;
pub use shadow::TextShadow;
pub use style::{TextColor, TextRole, TextStyle, TextStyled};
pub use widget::{Text, text};
