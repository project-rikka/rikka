use rikka_compositor_desktop::{Drawn, Target};
use rikka_compositor_geometry::{Logical, Physical, Point, Rect, Region, Scale};
use rikka_compositor_planes::Surface as Candidate;
use rikka_compositor_surface::{SurfaceId, Surfaces};
use rikka_renderer::{Renderer, Scene};
use rikka_types::RoundedRect;

use super::DragIcon;
use super::caption::PlacedCaption;
use super::corners::Corners;
use super::depth::Depth;
use super::edge::Edge;
use super::outline::OutlineNodes;
use super::pointer::{CURSOR_Z, Pointer};
use super::scenes::Composed;
use super::staged::Staged;
use super::staging::{Frosted, Leaving, Portrayed, Staging};
use super::textures::ClientTextures;
use super::tree::{Drawn as DrawnSurface, Placement, SurfaceTree};
use super::view::View;
use super::window_shape::Shaped;
use crate::glass::{Behind, Frosting, Occupant, Painted, Pane, Panes, hide};
use crate::outputs::Output;
use crate::policy::{Decor, Glass};
use crate::wayland::ViewId;

const ICON_Z: i32 = CURSOR_Z - 512;

#[derive(Debug, Clone, Copy)]
struct BackdropExtra {
    tree: usize,
    pane: usize,
    surface: Painted,
    rect: Rect<i32, Physical>,
    alpha: f32,
    clip: Option<RoundedRect>,
}

pub(super) struct Composition {
    pub(super) scene: Scene,
    pub(super) trees: Vec<SurfaceTree>,
    pub(super) icon: Option<SurfaceTree>,
    pub(super) cursor: Option<SurfaceTree>,
    pub(super) corners: Option<Corners>,
    pub(super) pointer: Option<Pointer>,
    pub(super) panes: Panes,
    outline: OutlineNodes,
    bands: Vec<(usize, Rect<i32, Physical>)>,
    moving: Vec<usize>,
    spare: Vec<SurfaceTree>,
    extras: Vec<BackdropExtra>,
    placed_panes: Vec<(usize, Pane)>,
    pub(super) staged: Staged,
    pub(super) composed: Composed,
    pub(super) covered: Region<Physical>,
}

struct Brush<'a> {
    view: View,
    decor: Decor,
    surfaces: &'a mut Surfaces,
    staging: &'a Staging,
    textures: &'a mut ClientTextures,
    renderer: &'a mut Renderer,
}

struct Chrome {
    shaped: Option<Shaped>,
    caption: Option<PlacedCaption>,
}

impl Composition {
    pub(super) fn new() -> Self {
        Self {
            scene: Scene::new(),
            trees: Vec::new(),
            icon: None,
            cursor: None,
            corners: None,
            pointer: None,
            panes: Panes::default(),
            outline: OutlineNodes::default(),
            bands: Vec::new(),
            moving: Vec::new(),
            spare: Vec::new(),
            extras: Vec::new(),
            placed_panes: Vec::new(),
            staged: Staged::default(),
            composed: Composed::default(),
            covered: Region::new(),
        }
    }

    pub(super) fn landed(&mut self) {
        if !self.scene.has_damage() {
            self.panes.landed();
        }
    }

    fn drawn_trees_iter(&self) -> impl Iterator<Item = &SurfaceTree> + '_ {
        self.trees.iter().chain(&self.icon).chain(&self.cursor)
    }

    pub(super) fn roots(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.drawn_trees().map(SurfaceTree::root)
    }

    pub(super) fn drawn_trees(&self) -> impl Iterator<Item = &SurfaceTree> + '_ {
        self.drawn_trees_iter()
            .filter(|tree| !tree.drawn().is_empty())
    }

    pub(super) fn surfaces(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.drawn_trees_iter()
            .flat_map(|tree| tree.drawn().iter().map(|drawn| drawn.surface))
    }

    pub(super) fn draw_trees(
        &mut self,
        view: Option<View>,
        decor: Decor,
        surfaces: &mut Surfaces,
        composed: &Composed,
        textures: &mut ClientTextures,
        renderer: &mut Renderer,
    ) {
        let staging = &composed.staging;
        let mut previous_trees =
            std::mem::replace(&mut self.trees, std::mem::take(&mut self.spare));
        previous_trees.reverse();
        self.moving.clear();
        self.extras.clear();
        self.panes.begin();
        self.outline.begin();
        if let Some(view) = view {
            self.staged.mark_seen(view);
            let mut brush = Brush {
                view,
                decor,
                surfaces: &mut *surfaces,
                staging,
                textures,
                renderer: &mut *renderer,
            };
            let mut depth = Depth::above(0);
            let mut lifted = Vec::new();
            for item in &composed.drawn {
                match *item {
                    Drawn::Surface { target, .. } if staging.hidden.contains(&target) => {}
                    Drawn::Surface { target, .. } if staging.lifted.contains(&target) => {
                        lifted.push(*item);
                    }
                    Drawn::Surface {
                        target,
                        root,
                        location,
                    } => self.draw_surface(
                        &mut brush,
                        &mut previous_trees,
                        target,
                        root,
                        location,
                        &mut depth,
                    ),
                    Drawn::Snapshot {
                        snapshot, geometry, ..
                    } => {
                        if let Some(shown) = staging.exiting.get(&snapshot).copied() {
                            let exiting = Leaving {
                                snapshot,
                                geometry,
                                shown,
                            };
                            self.draw_frozen(decor, &exiting, view, &mut depth);
                        }
                    }
                }
            }
            for leaving in &staging.leaving {
                self.draw_frozen(decor, leaving, view, &mut depth);
            }
            if let Some(frosted) = &staging.frosted {
                self.frost_over(frosted, view, &mut depth);
            }
            self.staged.draw_into(&mut self.scene, staging, &mut depth);
            let (tree, pane) = (self.trees.len(), self.panes.placed().len());
            for (index, rect, alpha) in self.staged.over_drawn() {
                self.extras.push(BackdropExtra {
                    tree,
                    pane,
                    surface: Painted::PanelOver(index),
                    rect,
                    alpha,
                    clip: None,
                });
            }
            for item in lifted {
                if let Drawn::Surface {
                    target,
                    root,
                    location,
                } = item
                {
                    self.draw_surface(
                        &mut brush,
                        &mut previous_trees,
                        target,
                        root,
                        location,
                        &mut depth,
                    );
                }
            }
        }
        self.staged.finish(&mut self.scene, staging);
        self.outline.finish(&mut self.scene);
        for mut gone in previous_trees.drain(..) {
            gone.clear(&mut self.scene);
        }
        self.spare = previous_trees;
        self.panes.finish(&mut self.scene, surfaces);
        self.bands.clear();
        self.bands.extend(self.panes.bands());
        let bands = std::mem::take(&mut self.bands);
        self.occlude(&bands, view.map(View::screen));
        self.bands = bands;
        self.update_glass();
        renderer.retain_backdrops(self.panes.holds());
    }

    fn draw_surface(
        &mut self,
        brush: &mut Brush<'_>,
        previous_trees: &mut Vec<SurfaceTree>,
        target: Target,
        root: SurfaceId,
        location: Point<i32, Logical>,
        depth: &mut Depth,
    ) {
        let view = brush.view;
        let moved = brush.staging.moved.get(&target).copied();
        let caption = match target {
            Target::Window(window) => brush.staging.captions.get(&window).cloned(),
            Target::Layer(_) | Target::Popup(_) | Target::Lock(_) => None,
        };
        let bounds = brush
            .surfaces
            .bounding_box(root)
            .map(|mut bounds| {
                bounds.loc += location;
                bounds
            })
            .map(|bounds| {
                caption
                    .as_ref()
                    .map_or(bounds, |caption| bounds.merge(caption.geometry))
            });
        if moved.is_none() && !bounds.is_some_and(|bounds| bounds.overlaps(view.area)) {
            return;
        }
        let mut tree = take_tree(previous_trees, root, None);
        tree.refresh(brush.surfaces, brush.textures, brush.renderer);
        if let Target::Window(window) = target
            && self
                .staged
                .beneath(&mut self.scene, brush.staging, window, depth)
        {
            let (tree, pane) = (self.trees.len(), self.panes.placed().len());
            for (index, rect, alpha) in self.staged.beneath_drawn() {
                self.extras.push(BackdropExtra {
                    tree,
                    pane,
                    surface: Painted::PanelBeneath(index),
                    rect,
                    alpha,
                    clip: None,
                });
            }
        }
        if moved.is_some() {
            self.moving.push(self.trees.len());
        }
        let placement = match moved {
            Some(moved) => moved.placement(location, view.area.loc, view.scale),
            None => Placement {
                origin: (location - view.area.loc).to_f64(),
                scale: view.scale.into(),
                alpha: 1.0,
                clip: None,
            },
        };
        let shaped = match target {
            Target::Window(window) => brush
                .staging
                .shapes
                .get(&window)
                .map(|shape| shape.place(&placement, location)),
            Target::Layer(_) | Target::Popup(_) | Target::Lock(_) => None,
        };
        let placement = Placement {
            clip: shaped.and_then(|shaped| shaped.clip()),
            ..placement
        };
        let frosting = match brush.staging.scrims.get(&target) {
            Some(&opacity) => Frosting::Scrim { opacity },
            None => Frosting::Glass(brush.decor.glass),
        };
        let caption = caption.map(|caption| caption.place(location, &placement));
        let chrome = Chrome { shaped, caption };
        self.draw_tree(brush, tree, placement, frosting, chrome, depth);
        self.draw_portrayed(brush, previous_trees, root, depth);
        let Some(shaped) = shaped else {
            return;
        };
        for outline in &brush.staging.outlines {
            if Target::Window(outline.window) == target
                && let Some((rect, alpha)) = self.outline.show(
                    &mut self.scene,
                    *outline,
                    shaped.area,
                    placement.alpha,
                    view,
                    depth,
                )
            {
                self.extras.push(BackdropExtra {
                    tree: self.trees.len(),
                    pane: self.panes.placed().len(),
                    surface: Painted::Outline(outline.window),
                    rect,
                    alpha,
                    clip: None,
                });
            }
        }
    }

    fn draw_portrayed(
        &mut self,
        brush: &mut Brush<'_>,
        previous_trees: &mut Vec<SurfaceTree>,
        parent: SurfaceId,
        depth: &mut Depth,
    ) {
        let Some(portrayed) = brush.staging.portrayed.get(&parent) else {
            return;
        };
        let view = brush.view;
        for seen in portrayed {
            let placement = seen
                .moved
                .placement(seen.drawn_at, view.area.loc, view.scale);
            let shaped = seen
                .shape
                .map(|shape| shape.place(&placement, seen.drawn_at));
            let Some(clip) = portrayed_clip(seen, shaped, view) else {
                continue;
            };
            let placement = Placement {
                clip: Some(clip),
                ..placement
            };
            let mut tree = take_tree(previous_trees, seen.root, Some(seen.view));
            tree.refresh(brush.surfaces, brush.textures, brush.renderer);
            let frosting = Frosting::Glass(brush.decor.glass);
            let caption = seen
                .caption
                .clone()
                .map(|caption| caption.place(seen.drawn_at, &placement));
            let chrome = Chrome { shaped, caption };
            self.draw_tree(brush, tree, placement, frosting, chrome, depth);
        }
    }

    fn draw_tree(
        &mut self,
        brush: &mut Brush<'_>,
        mut tree: SurfaceTree,
        placement: Placement,
        frosting: Frosting,
        Chrome { shaped, caption }: Chrome,
        depth: &mut Depth,
    ) {
        let view = tree.view();
        let surface_alpha = brush
            .surfaces
            .get(tree.root())
            .map_or(1.0, |surface| surface.alpha() as f32);
        let behind = Behind {
            origin: placement.origin,
            scale: placement.scale,
            density: brush.view.scale,
            alpha: placement.alpha,
            stack: self.trees.len(),
            view,
            clip: placement.clip,
        };
        self.panes.place(
            &mut self.scene,
            brush.surfaces,
            tree.root(),
            frosting,
            behind,
            depth,
        );
        let caption = caption.map(|caption| caption.with_alpha(surface_alpha));
        if let Some(caption) = caption
            .as_ref()
            .filter(|caption| caption.paint().translucent())
            && let Frosting::Glass(glass) = frosting
        {
            let band = (tree.caption_key(), caption.band());
            self.panes.frost(
                &mut self.scene,
                &[band],
                glass,
                Behind {
                    alpha: placement.alpha * surface_alpha,
                    ..behind
                },
                depth,
            );
        }
        tree.draw_caption(&mut self.scene, caption, depth);
        tree.draw(
            &mut self.scene,
            brush.surfaces,
            brush.textures,
            placement,
            depth,
        );
        let alpha = surface_alpha * placement.alpha;
        let edge = shaped
            .and_then(|shaped| shaped.edge(brush.decor.edge, alpha))
            .map(|edge| Edge {
                clip: view.and(placement.clip),
                ..edge
            });
        tree.draw_edge(
            &mut self.scene,
            shaped.map(|shaped| shaped.shape),
            edge,
            depth,
        );
        self.trees.push(tree);
    }

    fn draw_frozen(&mut self, decor: Decor, leaving: &Leaving, view: View, depth: &mut Depth) {
        self.frost_frozen(decor.glass, leaving, view, depth);
        self.staged
            .leaving(&mut self.scene, leaving, view, decor.edge, depth);
        let (tree, pane) = (self.trees.len(), self.panes.placed().len());
        for (index, rect, alpha, clip) in self.staged.frozen_drawn_for(leaving.snapshot) {
            self.extras.push(BackdropExtra {
                tree,
                pane,
                surface: Painted::Frozen(leaving.snapshot, index),
                rect,
                alpha,
                clip,
            });
        }
    }

    fn frost_frozen(&mut self, glass: Glass, leaving: &Leaving, view: View, depth: &mut Depth) {
        let Leaving {
            snapshot,
            geometry,
            shown,
        } = *leaving;
        let Some(frost) = self.staged.frost(snapshot) else {
            return;
        };
        let stretch = shown.stretch(geometry);
        let placed = shown.rect.loc - view.area.loc.to_f64();
        let behind = Behind {
            origin: Point::new(placed.x / stretch.x, placed.y / stretch.y),
            scale: Scale {
                x: view.scale * stretch.x,
                y: view.scale * stretch.y,
            },
            density: view.scale,
            alpha: shown.alpha,
            stack: self.trees.len(),
            view: None,
            clip: None,
        };
        self.panes
            .frost(&mut self.scene, frost, glass, behind, depth);
    }

    fn frost_over(&mut self, frosted: &Frosted, view: View, depth: &mut Depth) {
        let behind = Behind {
            origin: Point::new(-f64::from(view.area.loc.x), -f64::from(view.area.loc.y)),
            scale: Scale {
                x: view.scale,
                y: view.scale,
            },
            density: view.scale,
            alpha: frosted.alpha,
            stack: self.trees.len(),
            view: None,
            clip: None,
        };
        self.panes.frost(
            &mut self.scene,
            &frosted.panes,
            Glass::scrim(),
            behind,
            depth,
        );
    }

    fn update_glass(&mut self) {
        if self.panes.placed().is_empty() {
            self.panes.refresh_backdrops(&mut self.scene, &[]);
            return;
        }
        let mut placed = std::mem::take(&mut self.placed_panes);
        placed.clear();
        placed.extend_from_slice(self.panes.placed());
        let mut painted =
            Vec::with_capacity(placed.len() + self.extras.len() + self.trees.len() * 2);
        let mut pane_cursor = 0;
        let mut extra_cursor = 0;
        for stack in 0..=self.trees.len() {
            while extra_cursor < self.extras.len() && self.extras[extra_cursor].tree == stack {
                let extra = self.extras[extra_cursor];
                while pane_cursor < extra.pane && pane_cursor < placed.len() {
                    painted.push(Occupant::Pane(&placed[pane_cursor].1));
                    pane_cursor += 1;
                }
                painted.push(Occupant::Surface {
                    surface: extra.surface,
                    rect: extra.rect,
                    alpha: extra.alpha,
                    clip: extra.clip,
                    changed: &[],
                });
                extra_cursor += 1;
            }
            if let Some(tree) = self.trees.get(stack) {
                while pane_cursor < placed.len() && placed[pane_cursor].0 == stack {
                    painted.push(Occupant::Pane(&placed[pane_cursor].1));
                    pane_cursor += 1;
                }
                let view = tree.view();
                if let Some(caption) = tree.caption() {
                    painted.push(Occupant::Surface {
                        surface: Painted::Caption(tree.root(), view),
                        rect: caption.area(),
                        alpha: caption.alpha(),
                        clip: None,
                        changed: tree.caption_changed(),
                    });
                }
                painted.extend(tree.drawn().iter().map(|drawn| Occupant::Surface {
                    surface: Painted::Surface(drawn.surface, view),
                    rect: drawn.rect,
                    alpha: drawn.opacity(),
                    clip: drawn.clip(),
                    changed: &drawn.changed,
                }));
                if let Some((rect, alpha, clip)) = tree.edge_bounds() {
                    painted.push(Occupant::Surface {
                        surface: Painted::Edge(tree.root(), view),
                        rect,
                        alpha,
                        clip,
                        changed: &[],
                    });
                }
            } else {
                while pane_cursor < placed.len() {
                    painted.push(Occupant::Pane(&placed[pane_cursor].1));
                    pane_cursor += 1;
                }
            }
        }
        self.panes.refresh_backdrops(&mut self.scene, &painted);
        drop(painted);
        self.placed_panes = placed;
    }

    fn occlude(
        &mut self,
        bands: &[(usize, Rect<i32, Physical>)],
        screen: Option<Rect<i32, Physical>>,
    ) {
        let covered = &mut self.covered;
        covered.clear();
        for (stack, tree) in self.trees.iter_mut().enumerate().rev() {
            for index in (0..tree.drawn().len()).rev() {
                let drawn = &tree.drawn()[index];
                let outside = screen.is_some_and(|screen| !drawn.rect.overlaps(screen));
                let hidden = !drawn.rect.is_empty() && (outside || covered.covers(drawn.rect));
                hide(covered, &drawn.opaque, stack, bands);
                tree.show(&mut self.scene, index, !hidden);
            }
        }
    }

    pub(super) fn covering(
        &self,
        whole: Rect<i32, Physical>,
        surfaces: &Surfaces,
        textures: &ClientTextures,
    ) -> Option<Candidate<SurfaceId>> {
        if !self.scene.effects().is_empty() {
            return None;
        }
        self.trees
            .iter()
            .enumerate()
            .rev()
            .filter(|(stack, tree)| tree.view().is_none() && !self.moving.contains(stack))
            .flat_map(|(stack, tree)| tree.drawn().iter().rev().map(move |drawn| (stack, drawn)))
            .filter(|(_, drawn)| drawn.shown() && drawn.rect.contains_rect(whole))
            .find_map(|(stack, drawn)| self.candidate(stack, drawn, surfaces, textures))
    }

    fn candidate(
        &self,
        stack: usize,
        drawn: &DrawnSurface,
        surfaces: &Surfaces,
        textures: &ClientTextures,
    ) -> Option<Candidate<SurfaceId>> {
        if !drawn.unclipped() || drawn.opacity() < 1.0 {
            return None;
        }
        let opaque = {
            let mut region = Region::new();
            drawn.opaque.iter().for_each(|rect| region.add(*rect));
            region.covers(drawn.rect)
        };
        let format = textures.scanout_format(drawn.surface)?;
        let surface = surfaces.get(drawn.surface)?;
        let buffer = surface.buffer_size()?;
        Some(Candidate {
            id: drawn.surface,
            format,
            buffer,
            source: surface
                .source_in_buffer()
                .unwrap_or_else(|| Rect::from_size(buffer).to_f64()),
            transform: surface.transform(),
            placement: drawn.rect,
            opaque,
            covered: self.covered(stack, drawn),
            tearing: surface.tearing(),
        })
    }

    fn covered(&self, stack: usize, drawn: &DrawnSurface) -> bool {
        self.bands
            .iter()
            .any(|(above, band)| *above > stack && band.intersection(drawn.rect).is_some())
            || self
                .scene
                .painted_over(drawn.node(), drawn.rect.into())
                .is_some()
    }

    pub(super) fn draw_icon(
        &mut self,
        view: Option<View>,
        icon: Option<DragIcon>,
        surfaces: &mut Surfaces,
        textures: &mut ClientTextures,
        renderer: &mut Renderer,
    ) {
        let Some((view, icon)) = view.zip(icon) else {
            if let Some(mut tree) = self.icon.take() {
                tree.clear(&mut self.scene);
            }
            return;
        };
        let mut tree = match self.icon.take() {
            Some(tree) if tree.root() == icon.surface => tree,
            Some(mut other) => {
                other.clear(&mut self.scene);
                SurfaceTree::new(icon.surface)
            }
            None => SurfaceTree::new(icon.surface),
        };
        tree.refresh(surfaces, textures, renderer);
        let mut depth = Depth::above(ICON_Z);
        tree.draw(
            &mut self.scene,
            surfaces,
            textures,
            Placement {
                origin: icon.at - view.area.loc.to_f64(),
                scale: view.scale.into(),
                alpha: 1.0,
                clip: None,
            },
            &mut depth,
        );
        self.icon = Some(tree);
    }

    pub(super) fn draw_corners(&mut self, output: &Output, shown: bool) {
        let previous_corners = self.corners.take();
        let size = output.physical_size().filter(|_| shown);
        self.corners = match size {
            Some(size) => Corners::show(
                previous_corners,
                &mut self.scene,
                (size.w, size.h),
                output.scale(),
                CURSOR_Z - 1,
            ),
            None => {
                Corners::hide(previous_corners, &mut self.scene);
                None
            }
        };
    }
}

fn take_tree(trees: &mut Vec<SurfaceTree>, root: SurfaceId, view: Option<ViewId>) -> SurfaceTree {
    match trees
        .iter()
        .rposition(|tree| tree.root() == root && tree.view() == view)
    {
        Some(index) => trees.remove(index),
        None => SurfaceTree::through_view(root, view),
    }
}

fn portrayed_clip(seen: &Portrayed, shaped: Option<Shaped>, view: View) -> Option<RoundedRect> {
    let crop = RoundedRect::rect(view.exact(seen.moved.shown.rect));
    let crop = match shaped.and_then(|shaped| shaped.clip()) {
        Some(area) => crop.intersect(area)?,
        None => crop,
    };
    match seen.clip {
        Some(clip) => crop.intersect(view.shape(clip)),
        None => Some(crop),
    }
}
