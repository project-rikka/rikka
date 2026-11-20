use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Snapshot {
    pub(crate) fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba,
        }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = ((y * self.width + x) * 4) as usize;
        self.rgba.get(at..at + 4)?.try_into().ok()
    }
}

impl fmt::Debug for Snapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Snapshot")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("corner", &self.pixel(0, 0))
            .field("centre", &self.pixel(self.width / 2, self.height / 2))
            .finish_non_exhaustive()
    }
}
