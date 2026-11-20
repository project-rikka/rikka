use std::collections::VecDeque;

use rikka_image_edit::{Picture, Recipe, Resampling, Resize};

use crate::error::{Error, Result};

const LOGGED: usize = 256;

const KEPT: usize = 3;

const RESAMPLING: Resampling = Resampling::BicubicSmoother;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DirtyRegion {
    #[default]
    Nothing,
    Area([u32; 4]),
    Everything,
}

impl DirtyRegion {
    pub(crate) fn and(self, other: DirtyRegion) -> DirtyRegion {
        match (self, other) {
            (DirtyRegion::Nothing, region) | (region, DirtyRegion::Nothing) => region,
            (DirtyRegion::Everything, _) | (_, DirtyRegion::Everything) => DirtyRegion::Everything,
            (DirtyRegion::Area(one), DirtyRegion::Area(other)) => {
                let left = one[0].min(other[0]);
                let top = one[1].min(other[1]);
                let right = (one[0] + one[2]).max(other[0] + other[2]);
                let bottom = (one[1] + one[3]).max(other[1] + other[3]);
                DirtyRegion::Area([left, top, right - left, bottom - top])
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReductionKind {
    Shown,
    Thumbnail,
}

#[derive(Debug)]
struct CachedReduction {
    reduced: ReductionKind,
    extent: (u32, u32),
    revision: u64,
    picture: Picture,
}

#[derive(Debug, Default)]
pub(crate) struct Reductions {
    pending: DirtyRegion,
    log: VecDeque<(u64, DirtyRegion)>,
    cached: Vec<CachedReduction>,
}

impl Reductions {
    pub(crate) fn touch(&mut self, touched: DirtyRegion) {
        self.pending = self.pending.and(touched);
    }

    pub(crate) fn reached(&mut self, revision: u64) {
        let pending = std::mem::take(&mut self.pending);
        self.log.push_back((revision, pending));
        while self.log.len() > LOGGED {
            self.log.pop_front();
        }
    }

    pub(crate) fn reduce(
        &mut self,
        reduced: ReductionKind,
        extent: (u32, u32),
        revision: u64,
        source: impl FnOnce() -> Result<Picture>,
    ) -> Result<Picture> {
        let cached_index = self
            .cached
            .iter()
            .position(|entry| entry.reduced == reduced && entry.extent == extent);
        let earlier = cached_index.map(|index| self.cached.remove(index));
        let since = earlier.as_ref().map_or(DirtyRegion::Everything, |entry| {
            self.since(entry.revision, revision)
        });
        let picture = match (since, earlier) {
            (DirtyRegion::Nothing, Some(entry)) => entry.picture,
            (DirtyRegion::Area(area), Some(entry)) => {
                source()?.resized_since(&entry.picture, area, RESAMPLING)?
            }
            _ => resized(&source()?, extent)?,
        };
        if self.cached.len() >= KEPT {
            self.cached.remove(0);
        }
        self.cached.push(CachedReduction {
            reduced,
            extent,
            revision,
            picture: picture.clone(),
        });
        Ok(picture)
    }

    fn since(&self, cached_revision: u64, now: u64) -> DirtyRegion {
        let mut expected = cached_revision + 1;
        let mut dirty = DirtyRegion::Nothing;
        for (logged, more) in self
            .log
            .iter()
            .filter(|(logged, _)| *logged > cached_revision)
        {
            if *logged != expected {
                return DirtyRegion::Everything;
            }
            dirty = dirty.and(*more);
            expected += 1;
        }
        match expected == now + 1 {
            true => dirty,
            false => DirtyRegion::Everything,
        }
    }
}

pub(crate) fn resized(picture: &Picture, (width, height): (u32, u32)) -> Result<Picture> {
    if (width, height) == picture.dimensions() {
        return Ok(picture.clone());
    }
    picture
        .develop(&Recipe::new().then(Resize::new(width, height).resampling(RESAMPLING)))
        .map_err(Error::from)
}
