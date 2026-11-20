use ash::vk;
use rikka_types::{CornerRadii, Rect, RoundedRect};

use crate::scene::{Audience, Node, Scene, Shape};

const MIN_SIDE: f32 = 32.0;

const MIN_AREA: f32 = 96.0 * 96.0;

const MAX_PIECES: usize = 96;

const EDGE: f32 = 1.0;

const SAMPLED_EDGE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Position {
    pub(crate) segment: u32,
    pub(crate) chunk: u32,
    pub(crate) instance: u32,
}

impl Position {
    pub(crate) fn segment(segment: usize) -> Position {
        Position {
            segment: segment as u32,
            chunk: 0,
            instance: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Occluder {
    pub(crate) position: Position,
    pub(crate) cores: [Rect; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Piece {
    pub(crate) rect: Rect,
    pub(crate) floor: Option<Position>,
}

impl Piece {
    pub(crate) fn draws(&self, at: Position) -> bool {
        self.floor.is_none_or(|floor| at >= floor)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReblurRegion {
    pub(crate) segment: u32,
    pub(crate) reads: Rect,
}

pub(crate) fn may_occlude(node: &Node) -> bool {
    let solid = match node.shape {
        Shape::Rect | Shape::RoundedRect { .. } => node.backdrop.is_some() || node.fill.a >= 1.0,
        Shape::Image { rotation, .. } => {
            node.opaque.is_some() && rotation == 0.0 && node.fill.a >= 1.0
        }
        _ => false,
    };
    solid
        && node.visible
        && node.audience == Audience::Everyone
        && node.opacity >= 1.0
        && node.layer.is_none()
        && node.cutout.is_none()
        && node.rect.width() >= MIN_SIDE
        && node.rect.height() >= MIN_SIDE
        && node.rect.size.area() >= MIN_AREA
}

pub(crate) fn cores_of(scene: &Scene, node: &Node) -> Option<[Rect; 2]> {
    let radii = match node.shape {
        Shape::RoundedRect { radii } | Shape::Image { radii, .. } => radii,
        _ => CornerRadii::ZERO,
    };
    let inset = node.stroke.map_or(0.0, |stroke| stroke.width.max(0.0)) + EDGE;
    let offset = scene.offset_of(node.group);
    let shape = RoundedRect::new(node.rect.translate(offset), radii);
    let mut cores = bands(shape, inset);
    if cores[1] == cores[0] {
        cores[1] = Rect::ZERO;
    }
    let clip_offset = scene.offset_of(node.clip_group);
    let clips = [
        node.clip.map(|clip| RoundedRect {
            rect: clip.rect.translate(clip_offset),
            ..clip
        }),
        scene.group_clip_of(node),
    ];
    for clip in clips.into_iter().flatten() {
        if cores[1].is_empty() && clip.radii != CornerRadii::ZERO {
            let clip_bands = bands(clip, EDGE);
            let base = cores[0];
            cores = clip_bands.map(|band| base.intersection(&band).unwrap_or(Rect::ZERO));
        } else {
            let core = core_of(clip).unwrap_or(Rect::ZERO);
            cores = cores.map(|band| band.intersection(&core).unwrap_or(Rect::ZERO));
        }
    }
    if let Some(opaque) = node.opaque {
        let inner = opaque.translate(offset).inflate(-SAMPLED_EDGE);
        cores = cores.map(|band| band.intersection(&inner).unwrap_or(Rect::ZERO));
    }
    let mut cores = cores.map(|band| whole_inside(band).unwrap_or(Rect::ZERO));
    if cores[1] == cores[0] {
        cores[1] = Rect::ZERO;
    }
    cores.iter().any(|core| !core.is_empty()).then_some(cores)
}

fn bands(shape: RoundedRect, inset: f32) -> [Rect; 2] {
    let inner = shape.rect.inflate(-inset);
    shape
        .cores()
        .map(|band| band.intersection(&inner).unwrap_or(Rect::ZERO))
}

fn core_of(clip: RoundedRect) -> Option<Rect> {
    let inner = clip.rect.inflate(-EDGE);
    let reach = clip.radii.clamp_for_size(clip.rect.size).max() * RoundedRect::CORNER_REACH;
    let core = inner.inflate(-reach.min(inner.width().min(inner.height()) * 0.5));
    (!core.is_empty()).then_some(core)
}

fn whole_inside(rect: Rect) -> Option<Rect> {
    let (min_x, min_y) = (rect.min_x().ceil(), rect.min_y().ceil());
    let (max_x, max_y) = (rect.max_x().floor(), rect.max_y().floor());
    (max_x > min_x && max_y > min_y).then(|| Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

#[derive(Debug, Default)]
pub(crate) struct Occlusion {
    pub(crate) occluders: Vec<Occluder>,
    pub(crate) reblurred: Vec<ReblurRegion>,
    pub(crate) beneath: Vec<(Position, Rect)>,
    pub(crate) pieces: Vec<Piece>,
    pub(crate) draws: Vec<(u32, u32)>,
    pub(crate) clear_rects: Vec<vk::ClearRect>,
    remnants: Vec<Rect>,
}

impl Occlusion {
    pub(crate) fn clear(&mut self) {
        self.occluders.clear();
        self.reblurred.clear();
        self.beneath.clear();
        self.pieces.clear();
        self.draws.clear();
        self.clear_rects.clear();
    }

    pub(crate) fn keep_worthwhile(&mut self) {
        let beneath = &self.beneath;
        self.occluders.retain(|occluder| {
            beneath.iter().any(|(at, rect)| {
                *at < occluder.position && occluder.cores.iter().any(|core| core.intersects(rect))
            })
        });
    }

    pub(crate) fn cut(&mut self, damage: &[Rect]) {
        self.pieces.clear();
        self.pieces
            .extend(damage.iter().map(|&rect| Piece { rect, floor: None }));
        if self.occluders.is_empty() {
            return;
        }
        self.occluders
            .sort_unstable_by_key(|occluder| std::cmp::Reverse(occluder.position));
        let Self {
            occluders,
            reblurred,
            pieces,
            remnants,
            ..
        } = self;
        for occluder in occluders.iter() {
            for core in occluder.cores {
                if core.is_empty() {
                    continue;
                }
                let mut at = 0;
                while at < pieces.len() {
                    let piece = pieces[at];
                    at += 1;
                    if piece.floor.is_some() {
                        continue;
                    }
                    let Some(covered) = piece.rect.intersection(&core) else {
                        continue;
                    };
                    let held_back = reblurred.iter().any(|reblurred| {
                        reblurred.segment < occluder.position.segment
                            && reblurred.reads.intersects(&covered)
                    });
                    if held_back {
                        continue;
                    }
                    remnants.clear();
                    piece.rect.subtract(&core, remnants);
                    if pieces.len() + remnants.len() > MAX_PIECES {
                        return;
                    }
                    pieces[at - 1] = Piece {
                        rect: covered,
                        floor: Some(occluder.position),
                    };
                    pieces.extend(remnants.iter().map(|&rect| Piece { rect, floor: None }));
                }
            }
        }
    }
}
