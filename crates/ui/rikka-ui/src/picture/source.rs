use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_renderer::TextureId;
use rikka_types::Size;

use rikka_picture::{ComputedImage, Feed, ImportedBuffer, VideoController};

#[derive(Clone, Debug)]
pub enum VectorSource {
    Path(Arc<Path>),
    Bytes(Arc<[u8]>),
}

#[derive(Clone, Debug)]
pub struct Vector {
    document: VectorSource,
    size: Option<(u32, u32)>,
}

impl Vector {
    #[must_use]
    pub fn new(document: VectorSource) -> Vector {
        Vector {
            document,
            size: None,
        }
    }

    #[must_use]
    pub fn document(&self) -> &VectorSource {
        &self.document
    }

    #[must_use]
    pub fn size(&self) -> Option<(u32, u32)> {
        self.size
    }

    fn rastered_for(self, size: Size) -> Vector {
        match self.size {
            Some(_) => self,
            None => Vector {
                size: Some(extents(size)),
                ..self
            },
        }
    }
}

impl VectorSource {
    fn key(&self) -> VectorKey {
        match self {
            VectorSource::Path(path) => VectorKey::Path(Arc::clone(path)),
            VectorSource::Bytes(bytes) => VectorKey::Bytes(BufferKey::from_buffer(bytes)),
        }
    }
}

const VECTOR_EXTENSIONS: [&str; 2] = ["svg", "svgz"];

const EXACT_UP_TO: u32 = 256;

const RASTER_STEP: u32 = 64;

const MAX_EXTENT: u32 = 16_384;

#[derive(Clone, Debug)]
pub enum ImageSource {
    Path(Arc<Path>),
    Bytes(Arc<[u8]>),
    Vector(Vector),
    Rgba {
        width: u32,
        height: u32,
        pixels: Arc<[u8]>,
    },
    Feed(Feed),
    Computed(ComputedImage),
    Imported(ImportedBuffer),
    Texture {
        id: TextureId,
        size: Option<Size>,
    },
    Video(VideoController),
}

impl ImageSource {
    #[must_use]
    pub fn file(path: impl Into<Arc<Path>>) -> ImageSource {
        let path = path.into();
        let vector = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                VECTOR_EXTENSIONS
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known))
            });
        match vector {
            true => ImageSource::Vector(Vector::new(VectorSource::Path(path))),
            false => ImageSource::Path(path),
        }
    }

    #[must_use]
    pub fn vector(document: impl Into<Arc<[u8]>>) -> ImageSource {
        ImageSource::Vector(Vector::new(VectorSource::Bytes(document.into())))
    }

    #[must_use]
    pub fn rastered_for(self, size: Size) -> ImageSource {
        match self {
            ImageSource::Vector(vector) => ImageSource::Vector(vector.rastered_for(size)),
            settled => settled,
        }
    }

    #[must_use]
    pub fn natural_size(&self) -> Option<Size> {
        match self {
            ImageSource::Path(_) | ImageSource::Bytes(_) | ImageSource::Vector(_) => None,
            ImageSource::Rgba { width, height, .. } => {
                Some(Size::new(*width as f32, *height as f32))
            }
            ImageSource::Feed(feed) => feed.size(),
            ImageSource::Computed(computed) => computed.size(),
            ImageSource::Imported(imported) => imported.size(),
            ImageSource::Texture { size, .. } => *size,
            ImageSource::Video(controller) => controller.size(),
        }
    }

    #[must_use]
    pub fn key(&self) -> SourceKey {
        match self {
            ImageSource::Path(path) => SourceKey::Path(Arc::clone(path)),
            ImageSource::Bytes(bytes) => SourceKey::Bytes(BufferKey::from_buffer(bytes)),
            ImageSource::Rgba {
                width,
                height,
                pixels,
            } => SourceKey::Rgba {
                pixels: BufferKey::from_buffer(pixels),
                width: *width,
                height: *height,
            },
            ImageSource::Feed(feed) => SourceKey::Feed(feed.id()),
            ImageSource::Computed(computed) => SourceKey::Computed(computed.id()),
            ImageSource::Imported(imported) => SourceKey::Imported(imported.id()),
            ImageSource::Vector(vector) => SourceKey::Vector {
                document: vector.document.key(),
                size: vector.size,
            },
            ImageSource::Texture { id, .. } => SourceKey::Texture(*id),
            ImageSource::Video(controller) => SourceKey::Video(controller.id()),
        }
    }
}

fn extents(size: Size) -> (u32, u32) {
    let width = (size.width.ceil() as u32).clamp(1, MAX_EXTENT);
    let height = (size.height.ceil() as u32).clamp(1, MAX_EXTENT);
    let longest = width.max(height);
    let stepped = match longest <= EXACT_UP_TO {
        true => return (width, height),
        false => longest.div_ceil(RASTER_STEP) * RASTER_STEP,
    };
    let grown =
        |side: u32| (u64::from(side) * u64::from(stepped)).div_ceil(u64::from(longest)) as u32;
    (grown(width).max(1), grown(height).max(1))
}

impl PartialEq for ImageSource {
    fn eq(&self, other: &ImageSource) -> bool {
        self.key() == other.key()
    }
}

#[derive(Clone, Debug)]
pub struct BufferKey(Arc<[u8]>);

impl BufferKey {
    #[must_use]
    fn from_buffer(buffer: &Arc<[u8]>) -> BufferKey {
        BufferKey(Arc::clone(buffer))
    }

    fn is_sole_holder(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }
}

impl PartialEq for BufferKey {
    fn eq(&self, other: &BufferKey) -> bool {
        std::ptr::eq(Arc::as_ptr(&self.0), Arc::as_ptr(&other.0))
    }
}

impl Eq for BufferKey {}

impl Hash for BufferKey {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.0.as_ptr().addr().hash(hasher);
        self.0.len().hash(hasher);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VectorKey {
    Path(Arc<Path>),
    Bytes(BufferKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SourceKey {
    Path(Arc<Path>),
    Bytes(BufferKey),
    Rgba {
        pixels: BufferKey,
        width: u32,
        height: u32,
    },
    Vector {
        document: VectorKey,
        size: Option<(u32, u32)>,
    },
    Feed(u64),
    Computed(u64),
    Imported(u64),
    Texture(TextureId),
    Video(u64),
}

impl SourceKey {
    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        match self {
            SourceKey::Bytes(buffer)
            | SourceKey::Rgba { pixels: buffer, .. }
            | SourceKey::Vector {
                document: VectorKey::Bytes(buffer),
                ..
            } => buffer.is_sole_holder(),
            SourceKey::Path(_)
            | SourceKey::Vector {
                document: VectorKey::Path(_),
                ..
            }
            | SourceKey::Feed(_)
            | SourceKey::Computed(_)
            | SourceKey::Imported(_)
            | SourceKey::Texture(_)
            | SourceKey::Video(_) => false,
        }
    }
}

impl From<&str> for ImageSource {
    fn from(path: &str) -> ImageSource {
        ImageSource::file(Arc::<Path>::from(Path::new(path)))
    }
}

impl From<String> for ImageSource {
    fn from(path: String) -> ImageSource {
        ImageSource::file(Arc::<Path>::from(PathBuf::from(path)))
    }
}

impl From<&Path> for ImageSource {
    fn from(path: &Path) -> ImageSource {
        ImageSource::file(Arc::<Path>::from(path))
    }
}

impl From<PathBuf> for ImageSource {
    fn from(path: PathBuf) -> ImageSource {
        ImageSource::file(Arc::<Path>::from(path))
    }
}

impl From<Arc<Path>> for ImageSource {
    fn from(path: Arc<Path>) -> ImageSource {
        ImageSource::file(path)
    }
}

impl From<Feed> for ImageSource {
    fn from(feed: Feed) -> ImageSource {
        ImageSource::Feed(feed)
    }
}

impl From<ComputedImage> for ImageSource {
    fn from(computed: ComputedImage) -> ImageSource {
        ImageSource::Computed(computed)
    }
}

impl From<Vec<u8>> for ImageSource {
    fn from(bytes: Vec<u8>) -> ImageSource {
        ImageSource::Bytes(Arc::from(bytes))
    }
}

impl From<Arc<[u8]>> for ImageSource {
    fn from(bytes: Arc<[u8]>) -> ImageSource {
        ImageSource::Bytes(bytes)
    }
}

impl From<&VideoController> for ImageSource {
    fn from(controller: &VideoController) -> ImageSource {
        ImageSource::Video(controller.clone())
    }
}

impl From<TextureId> for ImageSource {
    fn from(id: TextureId) -> ImageSource {
        ImageSource::Texture { id, size: None }
    }
}

impl From<(TextureId, Size)> for ImageSource {
    fn from((id, size): (TextureId, Size)) -> ImageSource {
        ImageSource::Texture {
            id,
            size: Some(size),
        }
    }
}
