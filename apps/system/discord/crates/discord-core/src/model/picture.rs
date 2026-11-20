use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Picture {
    url: Arc<str>,
    sizing: Sizing,
    animated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sizing {
    Fixed,
    Query(&'static str),
}

const SMALLEST: u32 = 16;
const LARGEST: u32 = 4096;

impl Picture {
    #[must_use]
    pub fn fixed(url: impl Into<Arc<str>>) -> Picture {
        Picture {
            url: url.into(),
            sizing: Sizing::Fixed,
            animated: false,
        }
    }

    #[must_use]
    pub fn sized(url: impl Into<Arc<str>>, key: &'static str) -> Picture {
        Picture {
            url: url.into(),
            sizing: Sizing::Query(key),
            animated: false,
        }
    }

    #[must_use]
    pub fn animated(mut self, animated: bool) -> Picture {
        self.animated = animated;
        self
    }

    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.animated
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn url_for_size(&self, pixels: u32) -> String {
        match self.sizing {
            Sizing::Fixed => self.url.to_string(),
            Sizing::Query(key) => {
                let size = pixels.clamp(SMALLEST, LARGEST).next_power_of_two();
                let joint = if self.url.contains('?') { '&' } else { '?' };
                format!("{}{joint}{key}={size}", self.url)
            }
        }
    }
}
