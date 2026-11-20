use std::time::Duration;

use crate::raster::Image;

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    image: Image,
    delay: Duration,
}

impl Frame {
    #[must_use]
    pub const fn new(image: Image, delay: Duration) -> Frame {
        Frame { image, delay }
    }

    #[must_use]
    pub const fn image(&self) -> &Image {
        &self.image
    }

    #[must_use]
    pub const fn delay(&self) -> Duration {
        self.delay
    }

    #[must_use]
    pub fn into_image(self) -> Image {
        self.image
    }
}
