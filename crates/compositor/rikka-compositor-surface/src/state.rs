use std::rc::Rc;

use rikka_compositor_geometry::{Buffer, Damage, Logical, Point, Rect, Region, Size, Transform};
use rikka_compositor_protocols::wl_callback::WlCallback;
use rikka_compositor_protocols::wp_presentation_feedback::WpPresentationFeedback;
use rikka_compositor_types::{ContentType, TearingHint};
use smallvec::SmallVec;

use crate::blocker::Blockers;
use crate::buffer::Buffers;
use crate::cached::CachedStates;
use crate::color::ColorUpdate;
use crate::constraints::Constraints;
use crate::dispose::dispose;
use crate::ids::{BlockerId, BufferId, SurfaceId, UseId};
use crate::syncobj::SyncPoints;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    Add(SurfaceId),
    Above {
        child: SurfaceId,
        sibling: SurfaceId,
    },
    Below {
        child: SurfaceId,
        sibling: SurfaceId,
    },
}

#[derive(Debug, Default)]
pub(crate) struct Pending {
    pub(crate) buffer: Option<Option<BufferId>>,
    pub(crate) release_callbacks: Vec<WlCallback>,
    pub(crate) points: SyncPoints,
    pub(crate) update: Box<Update>,
}

impl Pending {
    pub(crate) fn discard(&mut self, buffers: &mut Buffers, blockers: &mut Blockers) {
        dispose(self.release_callbacks.drain(..));
        self.update.discard(buffers, blockers);
    }
}

#[derive(Debug, Default)]
pub(crate) struct Update {
    pub(crate) buffer: Option<Option<UseId>>,
    pub(crate) offset: Point<i32, Logical>,
    pub(crate) damage: Damage<Logical>,
    pub(crate) buffer_damage: Damage<Buffer>,
    pub(crate) opaque: Option<Option<Rc<Region<Logical>>>>,
    pub(crate) input: Option<Option<Rc<Region<Logical>>>>,
    pub(crate) scale: Option<i32>,
    pub(crate) transform: Option<Transform>,
    pub(crate) source: Option<Option<Rect<f64, Logical>>>,
    pub(crate) destination: Option<Option<Size<i32, Logical>>>,
    pub(crate) alpha: Option<u32>,
    pub(crate) content_type: Option<ContentType>,
    pub(crate) tearing: Option<TearingHint>,
    pub(crate) frame_callbacks: Vec<WlCallback>,
    pub(crate) feedback: Vec<WpPresentationFeedback>,
    pub(crate) placements: Vec<Placement>,
    pub(crate) positions: SmallVec<[(SurfaceId, Point<i32, Logical>); 1]>,
    pub(crate) fences: SmallVec<[BlockerId; 1]>,
    pub(crate) blockers: SmallVec<[BlockerId; 1]>,
    pub(crate) cached: CachedStates,
    pub(crate) constraints: Constraints,
    pub(crate) color: ColorUpdate,
}

impl Update {
    pub(crate) fn supersedes_feedback(&self) -> bool {
        self.buffer.is_some()
            || !self.damage.is_empty()
            || !self.buffer_damage.is_empty()
            || !self.feedback.is_empty()
    }

    pub(crate) fn position(&mut self, child: SurfaceId, at: Point<i32, Logical>) {
        match self
            .positions
            .iter_mut()
            .find(|(positioned, _)| *positioned == child)
        {
            Some((_, position)) => *position = at,
            None => self.positions.push((child, at)),
        }
    }

    pub(crate) fn absorb(
        &mut self,
        later: &mut Update,
        buffers: &mut Buffers,
        blockers: &mut Blockers,
    ) {
        let supersedes_feedback = later.supersedes_feedback();
        if let Some(buffer) = later.buffer.take() {
            if let Some(Some(superseded)) = self.buffer.replace(buffer) {
                buffers.drop_hold(superseded);
            }
            blockers.release_all(self.fences.drain(..));
            self.fences.extend(later.fences.drain(..));
        }
        self.offset += later.offset;
        self.damage.extend(later.damage.iter());
        self.buffer_damage.extend(later.buffer_damage.iter());
        take_over(&mut self.opaque, &mut later.opaque);
        take_over(&mut self.input, &mut later.input);
        take_over(&mut self.scale, &mut later.scale);
        take_over(&mut self.transform, &mut later.transform);
        take_over(&mut self.source, &mut later.source);
        take_over(&mut self.destination, &mut later.destination);
        take_over(&mut self.alpha, &mut later.alpha);
        take_over(&mut self.content_type, &mut later.content_type);
        take_over(&mut self.tearing, &mut later.tearing);
        self.frame_callbacks.append(&mut later.frame_callbacks);
        if supersedes_feedback {
            for superseded in self.feedback.drain(..) {
                superseded.discarded();
            }
            self.feedback.append(&mut later.feedback);
        }
        self.placements.append(&mut later.placements);
        for (child, at) in later.positions.drain(..) {
            self.position(child, at);
        }
        self.blockers.extend(later.blockers.drain(..));
        self.cached.absorb(&mut later.cached);
        self.constraints.absorb(later.constraints);
        self.color.absorb(&mut later.color);
        later.reset();
    }

    pub(crate) fn discard(&mut self, buffers: &mut Buffers, blockers: &mut Blockers) {
        if let Some(Some(buffer)) = self.buffer {
            buffers.drop_hold(buffer);
        }
        dispose(self.frame_callbacks.drain(..));
        for feedback in self.feedback.drain(..) {
            feedback.discarded();
        }
        blockers.release_all(self.fences.drain(..));
        blockers.release_all(self.blockers.drain(..));
        self.reset();
    }

    pub(crate) fn reset(&mut self) {
        self.buffer = None;
        self.offset = Point::zero();
        self.damage.clear();
        self.buffer_damage.clear();
        self.opaque = None;
        self.input = None;
        self.scale = None;
        self.transform = None;
        self.source = None;
        self.destination = None;
        self.alpha = None;
        self.content_type = None;
        self.tearing = None;
        self.frame_callbacks.clear();
        self.feedback.clear();
        self.placements.clear();
        self.positions.clear();
        self.fences.clear();
        self.blockers.clear();
        self.cached.clear();
        self.constraints = Constraints::default();
        self.color = ColorUpdate::default();
    }
}

fn take_over<T>(earlier: &mut Option<T>, later: &mut Option<T>) {
    if later.is_some() {
        *earlier = later.take();
    }
}
