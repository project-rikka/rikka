use std::cell::Cell;

use rikka_compositor_geometry::{Buffer, Logical, Point, Rect, Region, Size, Transform};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_protocols::wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1;
use rikka_compositor_protocols::wp_color_manager_v1::RenderIntent;
use rikka_compositor_protocols::wp_color_representation_surface_v1::WpColorRepresentationSurfaceV1;
use rikka_compositor_protocols::wp_fractional_scale_v1::WpFractionalScaleV1;
use rikka_compositor_protocols::wp_linux_drm_syncobj_surface_v1::WpLinuxDrmSyncobjSurfaceV1;
use rikka_compositor_protocols::wp_viewport::WpViewport;
use rikka_compositor_protocols::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1;
use rikka_compositor_types::OutputId;
use rikka_compositor_types::{ContentType, TearingHint};
use rikka_wayland_wire::ClientId;

use crate::color::{ColorRepresentation, ImageDescription};
use crate::current::Current;
use crate::damage::SurfaceDamage;
use crate::dmabuf::DmabufFeedback;
use crate::geometry::floor;
use crate::ids::{BufferId, SurfaceId};
use crate::latest::Latest;
use crate::role::RoleSlot;
use crate::state::{Pending, Update};

static NO_REGION: Region<Logical> = Region::new();

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Extent {
    Stale,
    Known(Option<Rect<i32, Logical>>),
}

#[derive(Debug, Default)]
pub(crate) struct Extensions {
    pub(crate) viewport: Option<WpViewport>,
    pub(crate) fractional: Option<WpFractionalScaleV1>,
    pub(crate) content_type: bool,
    pub(crate) alpha: bool,
    pub(crate) tearing: bool,
    pub(crate) fifo: bool,
    pub(crate) timer: bool,
    pub(crate) syncobj: Option<WpLinuxDrmSyncobjSurfaceV1>,
    pub(crate) color: bool,
    pub(crate) representation: Option<WpColorRepresentationSurfaceV1>,
    pub(crate) color_feedback: Vec<WpColorManagementSurfaceFeedbackV1>,
    pub(crate) dmabuf_feedback: Vec<ZwpLinuxDmabufFeedbackV1>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SurfaceHints {
    pub(crate) dmabuf: Option<DmabufFeedback>,
    pub(crate) description: Option<ImageDescription>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct Preferred {
    pub(crate) scale: Option<f64>,
    pub(crate) transform: Option<Transform>,
}

#[derive(Debug)]
pub struct Surface {
    pub(crate) resource: WlSurface,
    pub(crate) pending: Pending,
    pub(crate) cached: Option<Box<Update>>,
    pub(crate) latest: Latest,
    pub(crate) current: Current,
    pub(crate) role: Option<RoleSlot>,
    pub(crate) parent: Option<SurfaceId>,
    pub(crate) children: Vec<SurfaceId>,
    pub(crate) stack: Vec<SurfaceId>,
    pub(crate) position: Point<i32, Logical>,
    pub(crate) sync: bool,
    pub(crate) extensions: Extensions,
    pub(crate) preferred: Preferred,
    pub(crate) outputs: Vec<WlOutput>,
    pub(crate) main_output: Option<OutputId>,
    pub(crate) hints: SurfaceHints,
    pub(crate) extent: Cell<Extent>,
}

impl Surface {
    pub(crate) fn new(id: SurfaceId, resource: WlSurface) -> Self {
        Self {
            resource,
            pending: Pending::default(),
            cached: None,
            latest: Latest::default(),
            current: Current::default(),
            role: None,
            parent: None,
            children: Vec::new(),
            stack: vec![id],
            position: Point::zero(),
            sync: false,
            extensions: Extensions::default(),
            preferred: Preferred::default(),
            outputs: Vec::new(),
            main_output: None,
            hints: SurfaceHints::default(),
            extent: Cell::new(Extent::Stale),
        }
    }

    pub fn resource(&self) -> &WlSurface {
        &self.resource
    }

    pub fn client(&self) -> ClientId {
        self.resource.client()
    }

    pub fn role(&self) -> Option<&'static str> {
        self.role.as_ref().map(RoleSlot::name)
    }

    pub fn parent(&self) -> Option<SurfaceId> {
        self.parent
    }

    pub fn position(&self) -> Point<i32, Logical> {
        self.position
    }

    pub fn mapped(&self) -> bool {
        self.current.buffer.is_some()
    }

    pub fn buffer_attached(&self) -> bool {
        matches!(self.pending.buffer, Some(Some(_)))
            || self
                .cached
                .as_ref()
                .is_some_and(|cached| matches!(cached.buffer, Some(Some(_))))
            || self.current.buffer.is_some()
    }

    pub fn buffer(&self) -> Option<BufferId> {
        self.current.buffer.map(|attached| attached.buffer)
    }

    pub fn buffer_size(&self) -> Option<Size<i32, Buffer>> {
        self.current.buffer.map(|attached| attached.size)
    }

    pub fn size(&self) -> Option<Size<i32, Logical>> {
        self.current.size
    }

    pub fn scale(&self) -> i32 {
        self.current.scale
    }

    pub fn transform(&self) -> Transform {
        self.current.transform
    }

    pub fn source_in_buffer(&self) -> Option<Rect<f64, Buffer>> {
        self.current.geometry().source_in_buffer()
    }

    pub fn opaque(&self) -> &Region<Logical> {
        self.current
            .filled
            .as_deref()
            .or(self.current.opaque.as_deref())
            .unwrap_or(&NO_REGION)
    }

    pub fn input(&self) -> Option<&Region<Logical>> {
        self.current.input.as_deref()
    }

    pub fn accepts_input(&self, point: Point<f64, Logical>) -> bool {
        let Some(size) = self.current.size else {
            return false;
        };
        Rect::from_size(size.to_f64()).contains(point)
            && self
                .input()
                .is_none_or(|region| region.contains((floor(point.x), floor(point.y))))
    }

    pub fn alpha(&self) -> f64 {
        f64::from(self.current.alpha) / f64::from(u32::MAX)
    }

    pub fn has_alpha_modifier(&self) -> bool {
        self.extensions.alpha
    }

    pub fn content_type(&self) -> ContentType {
        self.current.content_type
    }

    pub fn tearing(&self) -> TearingHint {
        self.current.tearing
    }

    pub fn commits(&self) -> u64 {
        self.current.commits
    }

    pub fn damage(&self) -> &SurfaceDamage {
        &self.current.damage
    }

    pub fn awaits_frame(&self) -> bool {
        !self.current.frame_callbacks.is_empty()
    }

    pub fn outputs(&self) -> impl Iterator<Item = &WlOutput> {
        self.outputs.iter().filter(|output| output.alive())
    }

    pub fn image_description(&self) -> Option<(&ImageDescription, RenderIntent)> {
        self.current
            .color
            .description
            .as_ref()
            .map(|(description, intent)| (description, *intent))
    }

    pub fn color_representation(&self) -> ColorRepresentation {
        self.current.color.representation
    }

    pub fn main_output(&self) -> Option<OutputId> {
        self.main_output
    }
}
