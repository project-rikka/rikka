use std::rc::Rc;

use rikka_compositor_geometry::{Buffer, Logical, Rect, Region, Size, Transform};
use rikka_compositor_protocols::wl_callback::WlCallback;
use rikka_compositor_protocols::wp_presentation_feedback::WpPresentationFeedback;
use rikka_compositor_types::{ContentType, TearingHint};

use crate::buffer::Buffers;
use crate::cached::CachedStates;
use crate::color::ColorState;
use crate::damage::SurfaceDamage;
use crate::dispose::dispose;
use crate::geometry::Geometry;
use crate::ids::{BufferId, UseId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Attached {
    pub(crate) buffer: BufferId,
    pub(crate) size: Size<i32, Buffer>,
    pub(crate) use_id: Option<UseId>,
}

#[derive(Debug)]
pub(crate) struct Current {
    pub(crate) buffer: Option<Attached>,
    pub(crate) scale: i32,
    pub(crate) transform: Transform,
    pub(crate) source: Option<Rect<f64, Logical>>,
    pub(crate) destination: Option<Size<i32, Logical>>,
    pub(crate) opaque: Option<Rc<Region<Logical>>>,
    pub(crate) filled: Option<Rc<Region<Logical>>>,
    pub(crate) input: Option<Rc<Region<Logical>>>,
    pub(crate) alpha: u32,
    pub(crate) content_type: ContentType,
    pub(crate) tearing: TearingHint,
    pub(crate) frame_callbacks: Vec<WlCallback>,
    pub(crate) feedback: Vec<WpPresentationFeedback>,
    pub(crate) damage: SurfaceDamage,
    pub(crate) commits: u64,
    pub(crate) cached: CachedStates,
    pub(crate) color: ColorState,
    pub(crate) barrier: bool,
    pub(crate) size: Option<Size<i32, Logical>>,
}

impl Default for Current {
    fn default() -> Self {
        Self {
            buffer: None,
            scale: 1,
            transform: Transform::Normal,
            source: None,
            destination: None,
            opaque: None,
            filled: None,
            input: None,
            alpha: u32::MAX,
            content_type: ContentType::None,
            tearing: TearingHint::Vsync,
            frame_callbacks: Vec::new(),
            feedback: Vec::new(),
            damage: SurfaceDamage::default(),
            commits: 0,
            cached: CachedStates::default(),
            color: ColorState::default(),
            barrier: false,
            size: None,
        }
    }
}

impl Current {
    pub(crate) fn geometry(&self) -> Geometry {
        Geometry {
            buffer: self.buffer.map(|attached| attached.size),
            scale: self.scale,
            transform: self.transform,
            source: self.source,
            destination: self.destination,
        }
    }

    pub(crate) fn discard(&mut self, buffers: &mut Buffers) {
        if let Some(Attached {
            use_id: Some(use_id),
            ..
        }) = self.buffer.take()
        {
            buffers.drop_hold(use_id);
        }
        dispose(self.frame_callbacks.drain(..));
        for feedback in self.feedback.drain(..) {
            feedback.discarded();
        }
    }
}
