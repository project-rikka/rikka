use rikka_types::{Color, Rect, Size};
use rikka_ui::{Feed, ImageSource, ImportedBuffer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub(crate) u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BufferTransform {
    pub quarter_turns: u8,
    pub flipped: bool,
}

#[derive(Clone, Debug)]
pub enum LayerContent {
    Imported(ImportedBuffer),
    Feed(Feed),
    Solid(Color),
}

impl LayerContent {
    #[must_use]
    pub fn source(&self) -> Option<ImageSource> {
        match self {
            LayerContent::Imported(imported) => Some(ImageSource::Imported(imported.clone())),
            LayerContent::Feed(feed) => Some(ImageSource::Feed(feed.clone())),
            LayerContent::Solid(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub id: LayerId,
    pub rect: Rect,
    pub buffer: Size,
    pub crop: Option<Rect>,
    pub scale: f32,
    pub transform: BufferTransform,
    pub opaque: Vec<Rect>,
    pub alpha: f32,
    pub content: LayerContent,
}

#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub size: Size,
    pub layers: Vec<Layer>,
    pub generation: u64,
}
