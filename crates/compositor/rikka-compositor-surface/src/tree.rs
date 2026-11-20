use std::slice;

use rikka_compositor_geometry::{Logical, Point, Rect};
use smallvec::SmallVec;

use crate::ids::SurfaceId;
use crate::surface::Extent;
use crate::surfaces::Surfaces;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedSurface {
    pub surface: SurfaceId,
    pub location: Point<i32, Logical>,
}

#[derive(Debug)]
pub struct Tree<'a> {
    surfaces: &'a Surfaces,
    levels: SmallVec<[Level<slice::Iter<'a, SurfaceId>>; 4]>,
}

#[derive(Debug)]
struct Level<I> {
    surface: SurfaceId,
    location: Point<i32, Logical>,
    stack: I,
}

impl Iterator for Tree<'_> {
    type Item = PlacedSurface;

    fn next(&mut self) -> Option<PlacedSurface> {
        while let Some(level) = self.levels.last_mut() {
            let Some(&child_id) = level.stack.next() else {
                self.levels.pop();
                continue;
            };
            if child_id == level.surface {
                return Some(PlacedSurface {
                    surface: child_id,
                    location: level.location,
                });
            }
            let location = level.location;
            if let Some(child) = self.surfaces.surfaces.get(child_id)
                && child.mapped()
            {
                self.levels.push(Level {
                    surface: child_id,
                    location: location + child.position,
                    stack: child.stack.iter(),
                });
            }
        }
        None
    }
}

impl Surfaces {
    pub fn tree(&self, root: SurfaceId) -> Tree<'_> {
        let levels = self
            .surfaces
            .get(root)
            .filter(|surface| surface.mapped())
            .map(|surface| Level {
                surface: root,
                location: Point::zero(),
                stack: surface.stack.iter(),
            })
            .into_iter()
            .collect();
        Tree {
            surfaces: self,
            levels,
        }
    }

    pub fn flatten_into(&self, root: SurfaceId, placed: &mut Vec<PlacedSurface>) {
        placed.clear();
        placed.extend(self.tree(root));
    }

    pub fn surface_under(
        &self,
        root: SurfaceId,
        point: Point<f64, Logical>,
    ) -> Option<(SurfaceId, Point<f64, Logical>)> {
        let top = self.surfaces.get(root).filter(|surface| surface.mapped())?;
        let mut levels: SmallVec<[Level<std::iter::Rev<slice::Iter<'_, SurfaceId>>>; 4]> =
            SmallVec::new();
        levels.push(Level {
            surface: root,
            location: Point::zero(),
            stack: top.stack.iter().rev(),
        });
        while let Some(level) = levels.last_mut() {
            let Some(&child_id) = level.stack.next() else {
                levels.pop();
                continue;
            };
            let Some(node) = self.surfaces.get(child_id) else {
                continue;
            };
            if child_id == level.surface {
                let local = point - level.location.to_f64();
                if node.accepts_input(local) {
                    return Some((child_id, local));
                }
                continue;
            }
            if node.mapped() {
                let location = level.location + node.position;
                levels.push(Level {
                    surface: child_id,
                    location,
                    stack: node.stack.iter().rev(),
                });
            }
        }
        None
    }

    pub fn bounding_box(&self, root: SurfaceId) -> Option<Rect<i32, Logical>> {
        let surface = self.surfaces.get(root)?;
        if let Extent::Known(extent) = surface.extent.get() {
            return extent;
        }
        let extent = self
            .tree(root)
            .filter_map(|placed| {
                let size = self.surfaces.get(placed.surface)?.size()?;
                Some(Rect::new(placed.location, size))
            })
            .reduce(Rect::merge);
        surface.extent.set(Extent::Known(extent));
        extent
    }

    pub(crate) fn invalidate_extents(&self, surface: SurfaceId) {
        let mut current = Some(surface);
        while let Some(node) = current.and_then(|id| self.surfaces.get(id)) {
            node.extent.set(Extent::Stale);
            current = node.parent;
        }
    }
}
