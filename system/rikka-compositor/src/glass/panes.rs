use std::collections::{HashMap, HashSet};

use rikka_compositor_geometry::{Logical, Physical, Point, Rect, Scale};
use rikka_compositor_output::blur_region;
use rikka_compositor_surface::{SurfaceId, Surfaces};
use rikka_renderer::{BackdropCache, Blur, Node, NodeId, Scene};
use rikka_types::{Color, Rect as SceneRect, RoundedRect};

use super::joints;
use super::painted::Painted;
use super::pane_shapes::PaneShapes;
use super::placed::{Pane, Piece};
use super::repaint::{Occupant, Repaint};
use crate::policy::Glass;
use crate::render::Depth;
use crate::wayland::ViewId;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Frosting {
    Glass(Glass),
    Scrim { opacity: f32 },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Behind {
    pub(crate) origin: Point<f64, Logical>,
    pub(crate) scale: Scale<f64>,
    pub(crate) density: f64,
    pub(crate) alpha: f32,
    pub(crate) stack: usize,
    pub(crate) view: Option<ViewId>,
    pub(crate) clip: Option<RoundedRect>,
}

impl Behind {
    fn place(&self, shape: RoundedRect) -> RoundedRect {
        let (x, y) = (self.scale.x as f32, self.scale.y as f32);
        let left = ((self.origin.x as f32 + shape.rect.x()) * x).round();
        let top = ((self.origin.y as f32 + shape.rect.y()) * y).round();
        RoundedRect::new(
            SceneRect::new(left, top, shape.rect.width() * x, shape.rect.height() * y),
            shape.radii * x.min(y),
        )
    }
}

#[derive(Debug)]
struct PlacedPane {
    node: NodeId,
    pane: Pane,
    z: i32,
}

#[derive(Debug, Default)]
pub(crate) struct Panes {
    pane_shapes: HashMap<(SurfaceId, Option<ViewId>), PaneShapes>,
    viewed: HashSet<(SurfaceId, ViewId)>,
    shown: HashMap<u64, PlacedPane>,
    placed: Vec<(usize, Pane)>,
    repaint: Repaint<Painted>,
    holds: Vec<u64>,
    shapes: Vec<(u64, RoundedRect)>,
    pieces: Vec<(u64, Piece)>,
}

impl Panes {
    pub(crate) fn begin(&mut self) {
        self.placed.clear();
        self.viewed.clear();
    }

    pub(crate) fn place(
        &mut self,
        scene: &mut Scene,
        surfaces: &Surfaces,
        root: SurfaceId,
        frosting: Frosting,
        behind: Behind,
        depth: &mut Depth,
    ) {
        let Some(surface) = surfaces.get(root) else {
            return;
        };
        let alpha = behind.alpha * surface.alpha() as f32;
        let (glass, behind) = match frosting {
            Frosting::Glass(glass) => (glass, Behind { alpha, ..behind }),
            Frosting::Scrim { opacity } => (
                Glass::scrim(),
                Behind {
                    alpha: alpha * opacity,
                    ..behind
                },
            ),
        };
        if !glass.draws() || behind.alpha <= 0.0 {
            return;
        }
        let Some(size) = surface.size() else {
            return;
        };
        let region = blur_region(surfaces, root);
        if region.is_none() && matches!(frosting, Frosting::Glass(_)) {
            return;
        }
        if let Some(view) = behind.view {
            self.viewed.insert((root, view));
        }
        let shapes = self.pane_shapes.entry((root, behind.view)).or_default();
        let frost: Vec<(u64, RoundedRect)> = match region {
            Some(region) => shapes.panes(region, size).collect(),
            None => shapes.held().collect(),
        };
        self.frost(scene, &frost, glass, behind, depth);
    }

    pub(crate) fn frost(
        &mut self,
        scene: &mut Scene,
        frost: &[(u64, RoundedRect)],
        glass: Glass,
        behind: Behind,
        depth: &mut Depth,
    ) {
        if !glass.draws() || behind.alpha <= 0.0 {
            return;
        }
        let density = behind.density as f32;
        let radius = glass.radius() * density;
        let lens = glass.lens(density);
        self.shapes.clear();
        self.shapes.extend(frost.iter().filter_map(|&(key, shape)| {
            let placed = behind.place(shape);
            match behind.clip {
                Some(clip) => placed.intersect(clip).map(|shape| (key, shape)),
                None => Some((key, placed)),
            }
        }));
        self.pieces.clear();
        match (behind.view, behind.clip) {
            (Some(_), Some(_)) => self.pieces.extend(
                self.shapes
                    .iter()
                    .map(|&(key, shape)| (key, Piece::lone(shape))),
            ),
            _ => joints::pieces(&self.shapes, &mut self.pieces),
        }
        for &(key, piece) in &self.pieces {
            let z = depth.next();
            let pane = match self.shown.get(&key) {
                Some(placed) if placed.pane.cut_like(piece, radius, lens) => {
                    placed.pane.faded(behind.alpha)
                }
                _ => Pane::new(key, piece, radius, lens, behind.alpha),
            };
            show(&mut self.shown, scene, pane, z);
            self.placed.push((behind.stack, pane));
        }
    }

    pub(crate) fn finish(&mut self, scene: &mut Scene, surfaces: &Surfaces) {
        let placed = &self.placed;
        self.shown.retain(|key, shown| {
            let stays = placed.iter().any(|(_, pane)| pane.key == *key);
            if !stays {
                scene.remove(shown.node);
            }
            stays
        });
        let viewed = &self.viewed;
        self.pane_shapes.retain(|(surface, view), _| {
            surfaces.get(*surface).is_some()
                && view.is_none_or(|view| viewed.contains(&(*surface, view)))
        });
    }

    pub(crate) fn bands(&self) -> impl Iterator<Item = (usize, Rect<i32, Physical>)> + '_ {
        self.placed
            .iter()
            .map(|(stack, pane)| (*stack, pane.watched()))
    }

    pub(crate) fn placed(&self) -> &[(usize, Pane)] {
        &self.placed
    }

    pub(crate) fn refresh_backdrops(
        &mut self,
        scene: &mut Scene,
        painted: &[Occupant<'_, Painted>],
    ) {
        self.holds.clear();
        for stale in self.repaint.stale_regions(painted) {
            let Some(shown) = self.shown.get(&stale.key) else {
                continue;
            };
            let keep = match stale.stale.is_empty() {
                true => BackdropCache::current(stale.key),
                false => BackdropCache::stale(
                    stale.key,
                    stale.stale.iter().copied().map(SceneRect::from),
                ),
            };
            scene.edit(shown.node).backdrop_cache(keep);
            self.holds.push(stale.key);
        }
    }

    pub(crate) fn holds(&self) -> &[u64] {
        &self.holds
    }

    pub(crate) fn landed(&mut self) {
        self.repaint.landed();
    }
}

fn show(shown: &mut HashMap<u64, PlacedPane>, scene: &mut Scene, pane: Pane, z: i32) {
    match shown.get_mut(&pane.key) {
        Some(placed) => {
            let (was, now) = (placed.pane, pane.piece);
            let mut edit = scene.edit(placed.node);
            if was.piece.shape != now.shape {
                edit.rect(now.shape.rect).radii(now.shape.radii);
            }
            if was.piece.hollow != now.hollow {
                edit.cut_out(now.hollow);
            }
            if was.piece.joined != now.joined {
                edit.joined(now.joined);
            }
            if was.alpha != pane.alpha {
                edit.opacity(pane.alpha);
            }
            if was.lens != pane.lens {
                edit.lens(pane.lens);
            }
            if was.radius != pane.radius {
                edit.backdrop(Blur::with_radius(pane.radius));
            }
            if placed.z != z {
                edit.z(z);
            }
            drop(edit);
            placed.pane = pane;
            placed.z = z;
        }
        None => {
            let piece = pane.piece;
            let mut node = Node::rounded_rect(piece.shape.rect, piece.shape.radii)
                .fill(Color::TRANSPARENT)
                .opacity(pane.alpha)
                .backdrop(Blur::with_radius(pane.radius))
                .z(z);
            if let Some(lens) = pane.lens {
                node = node.lens(lens);
            }
            if let Some(hollow) = piece.hollow {
                node = node.cut_out(hollow);
            }
            if let Some(joined) = piece.joined {
                node = node.joined(joined);
            }
            let node = scene.insert(node);
            shown.insert(pane.key, PlacedPane { node, pane, z });
        }
    }
}
