use rikka_svg::ColorScheme;
use rikka_types::Source;

use crate::loader::handle::LoadHandle;
use crate::loader::task::{Decode, Task};
use crate::loader::{ImageLoader, submit};

#[derive(Debug)]
pub struct LoadRequest<'a> {
    loader: &'a ImageLoader,
    source: Source<'static>,
    vector: Option<(u32, u32)>,
    color_scheme: ColorScheme,
    thumbnail: Option<(u32, u32)>,
}

impl<'a> LoadRequest<'a> {
    pub(crate) fn new(loader: &'a ImageLoader, source: Source<'static>) -> LoadRequest<'a> {
        LoadRequest {
            loader,
            source,
            vector: None,
            color_scheme: ColorScheme::default(),
            thumbnail: None,
        }
    }

    #[must_use]
    pub fn vector(mut self, width: u32, height: u32) -> LoadRequest<'a> {
        self.vector = Some((width, height));
        self
    }

    #[must_use]
    pub fn color_scheme(mut self, scheme: ColorScheme) -> LoadRequest<'a> {
        self.color_scheme = scheme;
        self
    }

    #[must_use]
    pub fn thumbnail(mut self, max_width: u32, max_height: u32) -> LoadRequest<'a> {
        self.thumbnail = Some((max_width, max_height));
        self
    }

    pub fn submit(self) -> LoadHandle {
        let decode = match self.vector {
            Some((width, height)) => Decode::Vector {
                width,
                height,
                scheme: self.color_scheme,
            },
            None => Decode::Raster,
        };
        submit(
            self.loader,
            Task {
                source: self.source,
                decode,
                thumbnail: self.thumbnail,
            },
        )
    }
}
