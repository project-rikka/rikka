use rikka_compositor_geometry::{Logical, Physical, Point, Rect, Region, Scale, Size, Transform};
use rikka_compositor_surface::{PlacedSurface, SurfaceId, Surfaces};
use rikka_renderer::{Audience, ImageSource, Node, NodeId, Renderer, Scene, TextureId};
use rikka_types::{Rect as SceneRect, RoundedRect};

use super::caption::{CaptionNodes, PlacedCaption};
use super::depth::Depth;
use super::edge::{Edge, EdgeNode};
use super::textures::{ClientTextures, Look};
use super::window_shape::WindowShape;
use crate::wayland::ViewId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    pub(crate) origin: Point<f64, Logical>,
    pub(crate) scale: Scale<f64>,
    pub(crate) alpha: f32,
    pub(crate) clip: Option<RoundedRect>,
}

#[derive(Debug, Clone, Copy)]
struct Shape {
    look: Look,
    source: Option<(SceneRect, Transform)>,
}

impl Shape {
    fn redrawn_as(&self, next: &Shape) -> bool {
        self.source == next.source
            && match (self.look, next.look) {
                (Look::Texture(_, was), Look::Texture(_, now)) => was == now,
                (was, now) => was == now,
            }
    }

    fn swapped_texture(&self, next: &Shape) -> Option<TextureId> {
        match (self.look, next.look) {
            (Look::Texture(was, _), Look::Texture(now, _)) if was != now => Some(now),
            _ => None,
        }
    }

    fn bleed(&self, rect: Rect<i32, Physical>, scale: Scale<f64>) -> i32 {
        let stretch = match self.source {
            Some((source, transform)) => {
                let (across, down) = match transform.swaps_axes() {
                    true => (source.height(), source.width()),
                    false => (source.width(), source.height()),
                };
                (f64::from(rect.size.w) / f64::from(across).max(1.0))
                    .max(f64::from(rect.size.h) / f64::from(down).max(1.0))
            }
            None => scale.x.max(scale.y),
        };
        (stretch / 2.0).ceil().max(1.0) as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Place {
    rect: Rect<i32, Physical>,
    opacity: f32,
    clip: Option<RoundedRect>,
    z: i32,
    audience: Audience,
    generation: u64,
}

#[derive(Debug)]
enum ContentChange {
    Same,
    Damaged(Vec<Rect<i32, Physical>>),
    Whole,
}

#[derive(Debug)]
pub(crate) struct Drawn {
    pub(crate) surface: SurfaceId,
    node: NodeId,
    shape: Shape,
    pub(crate) rect: Rect<i32, Physical>,
    opacity: f32,
    clip: Option<RoundedRect>,
    z: i32,
    audience: Audience,
    seen: u64,
    shown: bool,
    pub(crate) opaque: Vec<Rect<i32, Physical>>,
    core: Option<Rect<i32, Physical>>,
    pub(crate) changed: Vec<Rect<i32, Physical>>,
}

impl Drawn {
    pub(crate) fn shown(&self) -> bool {
        self.shown
    }

    pub(crate) fn look(&self) -> (Look, Option<(SceneRect, Transform)>) {
        (self.shape.look, self.shape.source)
    }

    pub(crate) fn opacity(&self) -> f32 {
        self.opacity
    }

    pub(crate) fn node(&self) -> NodeId {
        self.node
    }

    pub(crate) fn clip(&self) -> Option<RoundedRect> {
        self.clip
    }

    pub(crate) fn unclipped(&self) -> bool {
        self.clip.is_none()
    }
}

#[derive(Debug)]
pub(crate) struct SurfaceTree {
    root: SurfaceId,
    view: Option<ViewId>,
    audience: Audience,
    members: Vec<PlacedSurface>,
    drawn: Vec<Drawn>,
    spare: Vec<Drawn>,
    shape: Option<WindowShape>,
    edge: EdgeNode,
    caption: CaptionNodes,
}

impl SurfaceTree {
    pub(crate) fn new(root: SurfaceId) -> Self {
        Self::through_view(root, None)
    }

    pub(crate) fn through_view(root: SurfaceId, view: Option<ViewId>) -> Self {
        Self {
            root,
            view,
            audience: Audience::Everyone,
            members: Vec::new(),
            drawn: Vec::new(),
            spare: Vec::new(),
            shape: None,
            edge: EdgeNode::default(),
            caption: CaptionNodes::default(),
        }
    }

    pub(crate) fn set_audience(&mut self, audience: Audience) {
        self.audience = audience;
    }

    pub(crate) fn root(&self) -> SurfaceId {
        self.root
    }

    pub(crate) fn view(&self) -> Option<ViewId> {
        self.view
    }

    pub(crate) fn drawn(&self) -> &[Drawn] {
        &self.drawn
    }

    pub(crate) fn shape(&self) -> Option<WindowShape> {
        self.shape
    }

    pub(crate) fn draw_edge(
        &mut self,
        scene: &mut Scene,
        shape: Option<WindowShape>,
        edge: Option<Edge>,
        depth: &mut Depth,
    ) {
        self.shape = shape;
        self.edge.draw(scene, edge, self.audience, depth);
    }

    pub(crate) fn draw_caption(
        &mut self,
        scene: &mut Scene,
        placed: Option<PlacedCaption>,
        depth: &mut Depth,
    ) {
        self.caption.draw(scene, placed, self.audience, depth);
    }

    pub(crate) fn caption(&self) -> Option<&PlacedCaption> {
        self.caption.placed()
    }

    pub(crate) fn caption_key(&mut self) -> u64 {
        self.caption.key()
    }

    pub(crate) fn caption_changed(&self) -> &[Rect<i32, Physical>] {
        self.caption.changed()
    }

    pub(crate) fn edge_bounds(&self) -> Option<(Rect<i32, Physical>, f32, Option<RoundedRect>)> {
        self.edge.bounds()
    }

    pub(crate) fn is_shown(&self) -> bool {
        self.drawn.iter().any(Drawn::shown)
    }

    pub(crate) fn members(&self) -> &[PlacedSurface] {
        &self.members
    }

    pub(crate) fn refresh(
        &mut self,
        surfaces: &mut Surfaces,
        textures: &mut ClientTextures,
        renderer: &mut Renderer,
    ) {
        surfaces.flatten_into(self.root, &mut self.members);
        for placed in &self.members {
            textures.refresh(placed.surface, surfaces, renderer);
        }
    }

    pub(crate) fn draw(
        &mut self,
        scene: &mut Scene,
        surfaces: &Surfaces,
        textures: &ClientTextures,
        placement: Placement,
        depth: &mut Depth,
    ) {
        let mut next_drawn = std::mem::take(&mut self.spare);
        next_drawn.clear();
        let unclipped = placement.clip.map(interior);
        for placed in &self.members {
            let Some(surface) = surfaces.get(placed.surface) else {
                continue;
            };
            let (Some(size), Some((look, generation))) =
                (surface.size(), textures.look(placed.surface))
            else {
                continue;
            };
            let origin = placement.origin + placed.location.to_f64();
            let rect = physical(origin, size.to_f64(), placement.scale);
            let shape = Shape {
                look,
                source: match look {
                    Look::Texture(..) => surface
                        .source_in_buffer()
                        .map(|source| (SceneRect::from(source), surface.transform())),
                    Look::Solid(_) => None,
                },
            };
            let opacity = surface.alpha() as f32 * placement.alpha;
            let opaque = match opacity >= 1.0 {
                true => within(
                    opaque_rects(surface.opaque(), origin, placement.scale, rect, look),
                    unclipped.as_deref(),
                ),
                false => Vec::new(),
            };
            let z = depth.next();
            let place = Place {
                rect,
                opacity,
                clip: placement.clip,
                z,
                audience: self.audience,
                generation,
            };
            let previous = self
                .drawn
                .iter()
                .position(|drawn| drawn.surface == placed.surface)
                .map(|index| self.drawn.swap_remove(index));
            let drawn = match previous {
                Some(previous) if previous.shape.redrawn_as(&shape) => {
                    let swapped = previous.shape.swapped_texture(&shape);
                    let change = match (generation == previous.seen, swapped) {
                        (true, None) => ContentChange::Same,
                        (true, Some(_)) => ContentChange::Whole,
                        (false, _) => match textures.damage_since(placed.surface, previous.seen) {
                            Some(damage) => {
                                let bleed = shape.bleed(rect, placement.scale);
                                ContentChange::Damaged(
                                    damage
                                        .iter()
                                        .filter_map(|damaged| {
                                            physical(
                                                origin + damaged.loc.to_f64(),
                                                damaged.size.to_f64(),
                                                placement.scale,
                                            )
                                            .grown(bleed)
                                            .intersection(rect)
                                        })
                                        .collect(),
                                )
                            }
                            None => ContentChange::Whole,
                        },
                    };
                    Drawn {
                        shape,
                        ..restyle(scene, previous, place, change, swapped)
                    }
                }
                Some(previous) => {
                    scene.remove(previous.node);
                    insert(scene, placed.surface, shape, place)
                }
                None => insert(scene, placed.surface, shape, place),
            };
            let core = opaque
                .iter()
                .copied()
                .max_by_key(|rect| i64::from(rect.size.w) * i64::from(rect.size.h));
            if drawn.core != core {
                scene
                    .edit(drawn.node)
                    .opaque_within(core.map(SceneRect::from));
            }
            next_drawn.push(Drawn {
                opaque,
                core,
                ..drawn
            });
        }
        for gone in self.drawn.drain(..) {
            scene.remove(gone.node);
        }
        self.spare = std::mem::replace(&mut self.drawn, next_drawn);
    }

    pub(crate) fn clear(&mut self, scene: &mut Scene) {
        for drawn in self.drawn.drain(..) {
            scene.remove(drawn.node);
        }
        self.edge.clear(scene);
        self.caption.clear(scene);
    }

    pub(crate) fn show(&mut self, scene: &mut Scene, index: usize, shown: bool) {
        let Some(drawn) = self.drawn.get_mut(index) else {
            return;
        };
        if drawn.shown != shown {
            drawn.shown = shown;
            scene.edit(drawn.node).visible(shown);
        }
    }
}

fn insert(scene: &mut Scene, surface: SurfaceId, shape: Shape, place: Place) -> Drawn {
    let Place {
        rect,
        opacity,
        clip,
        z,
        audience,
        generation,
    } = place;
    let placed = SceneRect::from(rect);
    let node = match shape.look {
        Look::Texture(texture, color) => {
            let source = shape
                .source
                .map_or_else(ImageSource::whole, |(region, transform)| {
                    ImageSource::whole().region(region).transform(transform)
                });
            Node::image(placed, texture).source(source.color(color))
        }
        Look::Solid(color) => Node::rect(placed).fill(color),
    };
    let node = match clip {
        Some(clip) => node.clip(clip),
        None => node,
    };
    Drawn {
        surface,
        node: scene.insert(node.opacity(opacity).z(z).audience(audience)),
        shape,
        rect,
        opacity,
        clip,
        z,
        audience,
        seen: generation,
        shown: true,
        opaque: Vec::new(),
        core: None,
        changed: vec![rect],
    }
}

fn restyle(
    scene: &mut Scene,
    mut drawn: Drawn,
    place: Place,
    change: ContentChange,
    swapped: Option<TextureId>,
) -> Drawn {
    let Place {
        rect,
        opacity,
        clip,
        z,
        audience,
        generation,
    } = place;
    if drawn.rect != rect {
        drawn.rect = rect;
        scene.edit(drawn.node).rect(SceneRect::from(rect));
    }
    if drawn.opacity != opacity {
        drawn.opacity = opacity;
        scene.edit(drawn.node).opacity(opacity);
    }
    if drawn.clip != clip {
        drawn.clip = clip;
        scene.edit(drawn.node).clip(clip);
    }
    if drawn.z != z {
        drawn.z = z;
        scene.edit(drawn.node).z(z);
    }
    if drawn.audience != audience {
        drawn.audience = audience;
        scene.edit(drawn.node).audience(audience);
    }
    drawn.changed = match change {
        ContentChange::Same => Vec::new(),
        ContentChange::Damaged(changed) => {
            let painted = changed.iter().copied().map(SceneRect::from);
            match swapped {
                Some(texture) => {
                    scene.edit(drawn.node).repaint_image(texture, painted);
                }
                None => {
                    scene.edit(drawn.node).repaint(painted);
                }
            }
            changed
        }
        ContentChange::Whole => {
            match swapped {
                Some(texture) => {
                    scene.edit(drawn.node).image(texture);
                }
                None => {
                    scene.edit(drawn.node).repaint([SceneRect::from(rect)]);
                }
            }
            vec![rect]
        }
    };
    drawn.seen = generation;
    drawn
}

fn opaque_rects(
    opaque: &Region<Logical>,
    origin: Point<f64, Logical>,
    scale: Scale<f64>,
    rect: Rect<i32, Physical>,
    look: Look,
) -> Vec<Rect<i32, Physical>> {
    if let Look::Solid(color) = look {
        return match color.a >= 1.0 {
            true => vec![rect],
            false => Vec::new(),
        };
    }
    opaque
        .iter()
        .filter_map(|area| {
            let inner = inside(origin + area.loc.to_f64(), area.size.to_f64(), scale)?;
            inner.intersection(rect)
        })
        .collect()
}

fn interior(clip: RoundedRect) -> Vec<Rect<i32, Physical>> {
    let mut cores: Vec<Rect<i32, Physical>> = clip
        .cores()
        .into_iter()
        .filter_map(covered_pixels)
        .collect();
    cores.dedup();
    cores
}

fn covered_pixels(rect: SceneRect) -> Option<Rect<i32, Physical>> {
    let left = rect.x().ceil() as i32;
    let top = rect.y().ceil() as i32;
    let right = rect.max_x().floor() as i32;
    let bottom = rect.max_y().floor() as i32;
    (right > left && bottom > top)
        .then(|| Rect::new(Point::new(left, top), Size::new(right - left, bottom - top)))
}

fn within(
    opaque: Vec<Rect<i32, Physical>>,
    unclipped: Option<&[Rect<i32, Physical>]>,
) -> Vec<Rect<i32, Physical>> {
    let Some(unclipped) = unclipped else {
        return opaque;
    };
    opaque
        .iter()
        .flat_map(|area| {
            unclipped
                .iter()
                .filter_map(|inner| area.intersection(*inner))
        })
        .collect()
}

pub(crate) fn physical(
    origin: Point<f64, Logical>,
    size: Size<f64, Logical>,
    scale: Scale<f64>,
) -> Rect<i32, Physical> {
    let left = (origin.x * scale.x).round() as i32;
    let top = (origin.y * scale.y).round() as i32;
    let right = ((origin.x + size.w) * scale.x).round() as i32;
    let bottom = ((origin.y + size.h) * scale.y).round() as i32;
    Rect::new(Point::new(left, top), Size::new(right - left, bottom - top))
}

fn inside(
    origin: Point<f64, Logical>,
    size: Size<f64, Logical>,
    scale: Scale<f64>,
) -> Option<Rect<i32, Physical>> {
    let left = (origin.x * scale.x).ceil() as i32;
    let top = (origin.y * scale.y).ceil() as i32;
    let right = ((origin.x + size.w) * scale.x).floor() as i32;
    let bottom = ((origin.y + size.h) * scale.y).floor() as i32;
    (right > left && bottom > top)
        .then(|| Rect::new(Point::new(left, top), Size::new(right - left, bottom - top)))
}
