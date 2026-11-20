use rikka_sync::Mutex;
use rikka_text::GlyphRaster;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::text::atlas::{AtlasKey, GlyphAtlas, GlyphRef};
use crate::text::routing::GlyphRoute;
use crate::text::workers::GlyphWorkers;
use rikka_gpu::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphRequest {
    key: AtlasKey,
    scale: u32,
}

impl GlyphRequest {
    fn from_route(route: &GlyphRoute) -> Self {
        Self {
            key: route.key,
            scale: route.scale.to_bits(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct GlyphHolder {
    index: FxHashMap<GlyphRequest, u32>,
    requests: Vec<(GlyphRoute, u32)>,
    picks: Vec<u32>,
    results: Vec<Option<GlyphRef>>,
    drawing: Vec<usize>,
    requested: FxHashSet<AtlasKey>,
}

pub(crate) struct GlyphSources<'a> {
    pub atlas: &'a Mutex<GlyphAtlas>,
    pub workers: &'a GlyphWorkers,
    pub raster: &'a mut GlyphRaster,
    pub completed: u64,
}

impl GlyphHolder {
    pub(crate) fn acquire_glyphs(
        &mut self,
        sources: GlyphSources<'_>,
        routes: &[GlyphRoute],
        refs: &mut Vec<Option<GlyphRef>>,
    ) -> Result<()> {
        refs.clear();
        self.gather(routes);
        if self.requests.is_empty() {
            return Ok(());
        }
        let GlyphSources {
            atlas,
            workers,
            raster,
            completed,
        } = sources;
        let mut missing = Vec::new();
        {
            let mut locked = atlas.lock();
            self.resolve(&mut locked, &mut missing);
        }
        if !missing.is_empty() {
            let drawn = workers.draw_all(raster, missing);
            let mut locked = atlas.lock();
            for drawn in drawn {
                if let Err(error) = locked.insert(drawn, completed) {
                    for (resolve, &(_, count)) in self.results.iter().zip(&self.requests) {
                        if let Some(key) = resolve.and_then(|resolve| resolve.placed_key()) {
                            locked.release_n(key, count);
                        }
                    }
                    return Err(error);
                }
            }
            for &at in &self.drawing {
                let (route, count) = self.requests[at];
                self.results[at] = locked.acquire_n(&route, count).map(|slot| GlyphRef {
                    key: route.key,
                    slot,
                });
            }
        }
        refs.extend(self.picks.iter().map(|&at| self.results[at as usize]));
        Ok(())
    }

    fn gather(&mut self, routes: &[GlyphRoute]) {
        self.index.clear();
        self.requests.clear();
        self.picks.clear();
        for route in routes {
            let requests = &mut self.requests;
            let at = *self
                .index
                .entry(GlyphRequest::from_route(route))
                .or_insert_with(|| {
                    requests.push((*route, 0));
                    (requests.len() - 1) as u32
                });
            self.requests[at as usize].1 += 1;
            self.picks.push(at);
        }
    }

    fn resolve(&mut self, atlas: &mut GlyphAtlas, missing: &mut Vec<AtlasKey>) {
        self.results.clear();
        self.drawing.clear();
        self.requested.clear();
        for (at, &(route, count)) in self.requests.iter().enumerate() {
            if let Some(slot) = atlas.acquire_n(&route, count) {
                self.results.push(Some(GlyphRef {
                    key: route.key,
                    slot,
                }));
                continue;
            }
            self.results.push(None);
            self.drawing.push(at);
            if self.requested.insert(route.key) {
                missing.push(route.key);
            }
        }
    }
}
