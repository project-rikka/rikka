use std::collections::{HashMap, HashSet};

use rikka_image_edit::{Depth, Imaging, Picture, Result};
use rikka_types::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Backdrop {
    Black,
    Clear,
    Deep,
}

#[derive(Default)]
pub(crate) struct Blanks {
    cache: HashMap<(u32, u32, Backdrop), Picture>,
    used: HashSet<(u32, u32, Backdrop)>,
}

impl Blanks {
    pub(crate) fn get(
        &mut self,
        imaging: &Imaging,
        width: u32,
        height: u32,
        backdrop: Backdrop,
    ) -> Result<Picture> {
        let key = (width, height, backdrop);
        self.used.insert(key);
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached.clone());
        }
        let (fill, depth) = match backdrop {
            Backdrop::Black => (Color::BLACK, Depth::Eight),
            Backdrop::Clear => (Color::TRANSPARENT, Depth::Eight),
            Backdrop::Deep => (Color::TRANSPARENT, Depth::Float),
        };
        let picture = imaging.blank(width, height, depth, fill)?;
        self.cache.insert(key, picture.clone());
        Ok(picture)
    }

    pub(crate) fn sweep(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.cache.retain(|key, _| used.contains(key));
    }
}
