use crate::capture::Capturer;
use crate::error::Result;
use crate::screen::Screen;
use crate::source::{Region, Source};

#[derive(Clone, Copy, Debug, Default)]
pub struct ShotOptions {
    pub cursor: bool,
    pub region: Option<Region>,
}

impl ShotOptions {
    pub fn cursor(mut self, include: bool) -> Self {
        self.cursor = include;
        self
    }

    pub fn region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }
}

#[derive(Clone)]
pub struct Shot {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Shot {
    pub fn take(source: &Source, options: ShotOptions) -> Result<Self> {
        Self::take_on(Screen::connect()?, source, options)
    }

    pub fn take_on(screen: Screen, source: &Source, options: ShotOptions) -> Result<Self> {
        let mut capturer = Capturer::open(screen, source, options.cursor)?;
        capturer.crop(options.region)?;

        let mut pixels = Vec::new();
        let (width, height) = capturer.capture_into(&mut pixels)?;
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
}

impl std::fmt::Debug for Shot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shot")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}
