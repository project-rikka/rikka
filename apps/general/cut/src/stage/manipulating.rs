use cut_project::{ClipId, Format, Redraw, Visual};
use rikka_app::prelude::*;
use rikka_figure::{
    Change, Frame, GrabTarget, Grip, Grips, Keys, Manipulation, ManipulationResult,
};
use rikka_ui::DRAG_SLOP;

use super::guides::{Guide, Targets};
use super::item_projection::ItemProjection;
use super::items::{Item, reached};
use super::viewing::Viewing;

const SNAP_REACH: f32 = 6.0;

#[derive(Clone, Debug)]
struct Member {
    clip: ClipId,
    visual: Visual,
    projection: ItemProjection,
}

#[derive(Clone, Debug)]
enum Operation {
    Moving {
        manipulation: Manipulation,
        bounds: Rect,
        from: Point,
    },
    Shaping {
        manipulation: Manipulation,
        projection: ItemProjection,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Manipulating {
    operation: Operation,
    members: Vec<Member>,
    reached: Vec<(ClipId, Visual)>,
    guides: Vec<Guide>,
    targets: Targets,
    origin: Point,
    moved: bool,
    solo: Option<ClipId>,
}

impl Member {
    fn from_item(item: &Item, format: Format) -> Option<Member> {
        item.visual.grips(format)?;
        Some(Member {
            clip: item.clip,
            visual: item.visual.clone(),
            projection: item.projection.clone()?,
        })
    }

    fn slid(&self, from: Point, to: Point) -> Visual {
        let (Some(start), Some(end)) =
            (self.projection.to_plane(from), self.projection.to_plane(to))
        else {
            return self.visual.clone();
        };
        let by = end - start;
        let placement = self.visual.placement();
        self.visual
            .placed(placement.slid(Point::new(by.x.round(), by.y.round())))
    }
}

impl Manipulating {
    pub(crate) fn moving(
        selected: &[&Item],
        items: &[Item],
        at: Point,
        origin: Point,
        format: Format,
        solo: Option<ClipId>,
    ) -> Option<Manipulating> {
        let bounds = selected
            .iter()
            .filter_map(|item| item.bounds(format))
            .reduce(|whole, one| whole.union(&one))?;
        let grips = Grips::Frame {
            frame: Frame::from_rect(bounds),
            sides: false,
            turns: false,
        };
        let manipulation = Manipulation::new(GrabTarget::Body, at, grips).fractional();
        Manipulating::new(
            Operation::Moving {
                manipulation,
                bounds,
                from: at,
            },
            selected,
            items,
            origin,
            format,
            solo,
        )
    }

    pub(crate) fn shaping(
        grip: Grip,
        (grips, projection): (Grips, ItemProjection),
        selected: &[&Item],
        items: &[Item],
        at: Point,
        origin: Point,
        format: Format,
    ) -> Option<Manipulating> {
        let flat = projection.to_plane(at)?;
        let manipulation = Manipulation::new(GrabTarget::Grip(grip), flat, grips);
        Manipulating::new(
            Operation::Shaping {
                manipulation,
                projection,
            },
            selected,
            items,
            origin,
            format,
            None,
        )
    }

    fn new(
        operation: Operation,
        selected: &[&Item],
        items: &[Item],
        origin: Point,
        format: Format,
        solo: Option<ClipId>,
    ) -> Option<Manipulating> {
        let members: Vec<Member> = selected
            .iter()
            .filter_map(|item| Member::from_item(item, format))
            .collect();
        if members.is_empty() {
            return None;
        }
        let others: Vec<Rect> = items
            .iter()
            .filter(|item| members.iter().all(|member| member.clip != item.clip))
            .filter_map(|item| item.bounds(format))
            .collect();
        let frame = Rect::new(0.0, 0.0, format.width as f32, format.height as f32);
        Some(Manipulating {
            reached: members
                .iter()
                .map(|member| (member.clip, member.visual.clone()))
                .collect(),
            members,
            operation,
            guides: Vec::new(),
            targets: Targets::new(frame, others),
            origin,
            moved: false,
            solo,
        })
    }

    pub(crate) fn reached(&self) -> &[(ClipId, Visual)] {
        &self.reached
    }

    pub(crate) fn guides(&self) -> &[Guide] {
        &self.guides
    }

    pub(crate) fn solo(&self) -> Option<ClipId> {
        self.solo
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.moved
    }

    pub(crate) fn redraw(&self) -> Redraw {
        match &self.operation {
            Operation::Moving { .. } => Redraw::Move,
            Operation::Shaping { manipulation, .. } => match manipulation.change() {
                Change::Move => Redraw::Move,
                Change::Resize => Redraw::Resize,
                Change::Rotate => Redraw::Rotate,
            },
        }
    }

    pub(crate) fn drag(
        &mut self,
        at: Point,
        position: Point,
        keys: Keys,
        snaps: bool,
        viewing: &Viewing,
        format: Format,
    ) -> bool {
        if !self.moved && self.origin.distance(position) < DRAG_SLOP {
            return false;
        }
        self.moved = true;
        self.guides.clear();
        match &self.operation {
            Operation::Moving {
                manipulation,
                bounds,
                from,
            } => {
                let ManipulationResult::Mapped { map, .. } = manipulation.drag_to(at, keys) else {
                    return true;
                };
                let mut by = Point::new(map.tx as f32, map.ty as f32);
                if snaps && !keys.shift {
                    let (pull, guides) = self.targets.pull(
                        bounds.translate(by),
                        SNAP_REACH / viewing.scale.max(f32::EPSILON),
                    );
                    by += pull;
                    self.guides = guides;
                }
                let (from, to) = (*from, *from + by);
                self.reached = self
                    .members
                    .iter()
                    .map(|member| (member.clip, member.slid(from, to)))
                    .collect();
            }
            Operation::Shaping {
                manipulation,
                projection,
            } => {
                let Some(flat) = projection.to_plane(at) else {
                    return true;
                };
                let originals: Vec<(ClipId, Visual)> = self
                    .members
                    .iter()
                    .map(|member| (member.clip, member.visual.clone()))
                    .collect();
                self.reached = reached(&originals, manipulation.drag_to(flat, keys), format);
            }
        }
        true
    }
}
