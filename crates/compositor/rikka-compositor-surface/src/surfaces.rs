use std::fmt;
use std::rc::Rc;
use std::time::Duration;

use rikka_compositor_geometry::{Logical, Region};
use rikka_compositor_protocols::wl_region::WlRegion;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_types::{Fourcc, OutputId};
use slotmap::SlotMap;

use crate::blocker::{Blocker, Blockers};
use crate::buffer::Buffers;
use crate::cached::{AppliedHook, Cached};
use crate::color::ColorManagement;
use crate::committed::Committed;
use crate::dmabuf::DmabufState;
use crate::ids::{BlockerId, SurfaceId};
use crate::protocol::objects::ProtocolObjects;
use crate::role::{RoleError, RoleHooks, RoleSlot};
use crate::state::Update;
use crate::surface::Surface;
use crate::syncobj::SyncobjDevice;
use crate::transaction::Queue;

const REQUIRED_SHM_FORMATS: [Fourcc; 2] = [Fourcc::ARGB8888, Fourcc::XRGB8888];

pub struct Surfaces {
    pub(crate) surfaces: SlotMap<SurfaceId, Surface>,
    pub(crate) buffers: Buffers,
    pub(crate) blockers: Blockers,
    pub(crate) queue: Queue,
    pub(crate) applied: Vec<(SurfaceId, AppliedHook)>,
    #[expect(
        clippy::vec_box,
        reason = "pooled updates travel between transactions as boxes"
    )]
    pub(crate) spare: Vec<Box<Update>>,
    pub(crate) walk: Vec<(SurfaceId, bool)>,
    pub(crate) commits: Vec<Committed>,
    pub(crate) awaited: Vec<OutputId>,
    pub(crate) barriers: Vec<SurfaceId>,
    pub(crate) members: Vec<SurfaceId>,
    pub(crate) horizons: Vec<(OutputId, Duration)>,
    pub(crate) dmabuf: DmabufState,
    pub(crate) syncobj: Option<Rc<SyncobjDevice>>,
    pub(crate) color: ColorManagement,
    pub(crate) objects: ProtocolObjects,
    shm_formats: Vec<Fourcc>,
}

impl Surfaces {
    pub fn new() -> Self {
        Self {
            surfaces: SlotMap::with_key(),
            buffers: Buffers::default(),
            blockers: Blockers::default(),
            queue: Queue::default(),
            applied: Vec::new(),
            spare: Vec::new(),
            walk: Vec::new(),
            commits: Vec::new(),
            awaited: Vec::new(),
            barriers: Vec::new(),
            members: Vec::new(),
            horizons: Vec::new(),
            dmabuf: DmabufState::default(),
            syncobj: None,
            color: ColorManagement::default(),
            objects: ProtocolObjects::default(),
            shm_formats: REQUIRED_SHM_FORMATS.to_vec(),
        }
    }

    pub fn set_shm_formats(&mut self, formats: impl IntoIterator<Item = Fourcc>) {
        let mut chosen = REQUIRED_SHM_FORMATS.to_vec();
        for format in formats {
            if !chosen.contains(&format) {
                chosen.push(format);
            }
        }
        self.shm_formats = chosen;
    }

    pub fn shm_formats(&self) -> &[Fourcc] {
        &self.shm_formats
    }

    pub fn get(&self, surface: SurfaceId) -> Option<&Surface> {
        self.surfaces.get(surface)
    }

    pub fn region(&self, region: &WlRegion) -> Rc<Region<Logical>> {
        crate::protocol::region::snapshot(&self.objects.regions, region)
    }

    pub fn lookup(&self, surface: &WlSurface) -> Option<SurfaceId> {
        surface
            .key::<SurfaceId>()
            .filter(|id| self.surfaces.contains_key(*id))
    }

    pub fn root(&self, surface: SurfaceId) -> SurfaceId {
        let mut root = surface;
        while let Some(parent) = self.surfaces.get(root).and_then(|surface| surface.parent) {
            root = parent;
        }
        root
    }

    pub fn role<R: 'static>(&self, surface: SurfaceId) -> Option<&R> {
        self.surfaces.get(surface)?.role.as_ref()?.payload::<R>()
    }

    pub fn role_mut<R: 'static>(&mut self, surface: SurfaceId) -> Option<&mut R> {
        self.surfaces
            .get_mut(surface)?
            .role
            .as_mut()?
            .payload_mut::<R>()
    }

    pub fn end_role<R: 'static>(&mut self, surface: SurfaceId) -> Option<R> {
        self.surfaces.get_mut(surface)?.role.as_mut()?.end::<R>()
    }

    pub fn block<B: Blocker>(
        &mut self,
        surface: SurfaceId,
        make: impl FnOnce(BlockerId) -> B,
    ) -> Option<BlockerId> {
        let surface = self.surfaces.get_mut(surface)?;
        let blocker = self.blockers.insert(make);
        surface.pending.update.blockers.push(blocker);
        Some(blocker)
    }

    pub fn stage<S: 'static, T: Cached<S>>(&mut self, surface: SurfaceId, value: T) {
        if let Some(surface) = self.surfaces.get_mut(surface) {
            surface.pending.update.cached.stage::<S, T>(value);
        }
    }

    pub fn cached<T: 'static>(&self, surface: SurfaceId) -> Option<&T> {
        self.surfaces.get(surface)?.current.cached.get::<T>()
    }

    pub(crate) fn next_applied(&mut self, surface: SurfaceId) -> Option<AppliedHook> {
        let at = self.applied.iter().position(|(id, _)| *id == surface)?;
        Some(self.applied.remove(at).1)
    }

    pub(crate) fn assign(&mut self, surface: SurfaceId, slot: RoleSlot) -> Result<(), RoleError> {
        let surface = self.surfaces.get_mut(surface).ok_or(RoleError::Gone)?;
        match surface.role.as_mut() {
            Some(role) => role.replace(slot),
            None => {
                surface.role = Some(slot);
                Ok(())
            }
        }
    }

    pub(crate) fn hooks<S: 'static>(&self, surface: SurfaceId) -> Option<RoleHooks<S>> {
        self.surfaces.get(surface)?.role.as_ref()?.hooks::<S>()
    }

    pub(crate) fn insert(&mut self, resource: WlSurface) -> SurfaceId {
        self.surfaces
            .insert_with_key(|id| Surface::new(id, resource))
    }

    pub(crate) fn synced(&self, surface: SurfaceId) -> bool {
        let mut current = surface;
        while let Some(node) = self.surfaces.get(current) {
            let Some(parent) = node.parent else {
                return false;
            };
            if node.sync {
                return true;
            }
            current = parent;
        }
        false
    }

    pub(crate) fn is_ancestor(&self, ancestor: SurfaceId, of: SurfaceId) -> bool {
        let mut current = Some(of);
        while let Some(node) = current {
            if node == ancestor {
                return true;
            }
            current = self.surfaces.get(node).and_then(|surface| surface.parent);
        }
        false
    }
}

impl Default for Surfaces {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Surfaces {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Surfaces")
            .field("surfaces", &self.surfaces.len())
            .field("queued", &self.queue.len())
            .field("shm_formats", &self.shm_formats)
            .finish_non_exhaustive()
    }
}
