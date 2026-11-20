use std::borrow::Cow;
use std::sync::Arc;

use crate::id::model_id;
use crate::mime;

model_id!(ImageId);
model_id!(TextureId);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    Nearest,
    #[default]
    Linear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Wrap {
    #[default]
    Repeat,
    Clamp,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sampler {
    pub filter: Filter,
    pub wrap: Wrap,
}

impl Sampler {
    #[must_use]
    pub fn new(filter: Filter, wrap: Wrap) -> Self {
        Self { filter, wrap }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

impl Image {
    pub fn new(name: impl Into<String>, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            name: name.into(),
            bytes: bytes.into(),
        }
    }

    #[must_use]
    pub fn file_name(&self, index: usize) -> Cow<'_, str> {
        match self.name.is_empty() {
            true => Cow::Owned(format!(
                "image{index}.{}",
                mime::extension(mime::guess(&self.bytes, &self.name))
            )),
            false => Cow::Borrowed(&self.name),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    pub name: String,
    pub image: ImageId,
    pub sampler: Sampler,
}

impl Texture {
    pub fn new(image: ImageId) -> Self {
        Self {
            name: String::new(),
            image,
            sampler: Sampler::default(),
        }
    }

    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    #[must_use]
    pub fn sampler(mut self, sampler: Sampler) -> Self {
        self.sampler = sampler;
        self
    }
}
