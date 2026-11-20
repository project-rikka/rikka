use std::collections::BTreeMap;
use std::sync::Arc;

use rikka_image_edit::Picture;

use crate::error::{Error, Result};
use crate::figure::{self, Figure, FigureId};
use crate::state::picture_bytes;

pub(crate) const PLAIN: u32 = 0;
const FIGURE_BYTES: u64 = 256;

#[derive(Clone, Debug)]
pub(crate) struct Overlay {
    pub(crate) base: Picture,
    pub(crate) figures: Vec<Figure>,
    pub(crate) linear: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Overlays(BTreeMap<u32, Arc<Overlay>>);

impl Overlay {
    pub(crate) fn baked(&self) -> Result<Picture> {
        baked(&self.base, &self.figures)
    }
}

pub(crate) fn baked(base: &Picture, figures: &[Figure]) -> Result<Picture> {
    if figures.is_empty() {
        return Ok(base.clone());
    }
    base.develop(&figure::recipe(figures)).map_err(Error::from)
}

impl Overlays {
    pub(crate) fn get(&self, key: u32) -> Option<&Overlay> {
        self.0.get(&key).map(Arc::as_ref)
    }

    pub(crate) fn set(&mut self, key: u32, overlay: Option<Overlay>) {
        match overlay {
            Some(overlay) => {
                self.0.insert(key, Arc::new(overlay));
            }
            None => {
                self.0.remove(&key);
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, &Overlay)> {
        self.0.iter().map(|(key, overlay)| (*key, overlay.as_ref()))
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(u32) -> bool) {
        self.0.retain(|key, _| keep(*key));
    }

    pub(crate) fn owner(&self, id: FigureId) -> Option<u32> {
        self.0
            .iter()
            .find(|(_, overlay)| overlay.figures.iter().any(|figure| figure.id == id))
            .map(|(key, _)| *key)
    }

    pub(crate) fn departed_bytes(&self, now: &Overlays) -> u64 {
        self.0
            .iter()
            .filter_map(|(key, overlay)| {
                let current = now.0.get(key);
                if current.is_some_and(|current_overlay| Arc::ptr_eq(overlay, current_overlay)) {
                    return None;
                }
                let base = match current {
                    Some(_) => 0,
                    None => picture_bytes(&overlay.base),
                };
                Some(overlay.figures.len() as u64 * FIGURE_BYTES + base)
            })
            .sum()
    }
}

impl FromIterator<(u32, Overlay)> for Overlays {
    fn from_iter<I: IntoIterator<Item = (u32, Overlay)>>(entries: I) -> Overlays {
        Overlays(
            entries
                .into_iter()
                .map(|(key, overlay)| (key, Arc::new(overlay)))
                .collect(),
        )
    }
}
