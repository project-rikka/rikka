pub(crate) mod client;

use std::num::NonZeroU32;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use rikka_compositor_geometry::{Point, Rect, Region, Size, Transform};
use rikka_compositor_output::{
    BackgroundEffectHost, BackgroundEffectProtocols, BackgroundEffects, CaptureFormats,
    CaptureHost, CaptureProtocols, CaptureRequest, CaptureSessionId, Captures, Constraints,
    DrmLeaseHost, DrmLeaseProtocols, DrmLeases, FrameReady, LeaseDeviceId, LeaseGrant,
    LeaseRejected, OutputHost, OutputProtocols, Outputs, SessionInfo,
};
use rikka_compositor_protocols::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use rikka_compositor_protocols::ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_pointer::WlPointer;
use rikka_compositor_protocols::wl_seat::{self, WlSeat};
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_server::{Client, ClientOptions, DisplayHandle};
use rikka_compositor_surface::{BufferContent, SurfaceHost, SurfaceId, SurfaceProtocols, Surfaces};
use rikka_compositor_types::{Fourcc, OutputId};
use rikka_test_display::{Display, Host, Notes};
use rustix::fs::{MemfdFlags, memfd_create};

pub(crate) use client::{Wayland, label};

pub(crate) const TOPLEVEL: u32 = 7;
pub(crate) const PRESENTED: Duration = Duration::new(0x1_0000_0002, 3);

pub(crate) struct State {
    pub(crate) display: DisplayHandle<State>,
    pub(crate) surfaces: Surfaces,
    pub(crate) outputs: Outputs,
    pub(crate) captures: Captures<u32>,
    pub(crate) effects: BackgroundEffects,
    pub(crate) leases: DrmLeases,
    pub(crate) bound_outputs: Vec<OutputId>,
    pub(crate) blur_commits: Vec<SurfaceId>,
    pub(crate) started: Vec<(CaptureSessionId, SessionInfo<u32>)>,
    pub(crate) requests: Vec<CaptureRequest<u32>>,
    pub(crate) offered: Option<Constraints>,
    pub(crate) copy_with: Option<u8>,
    pub(crate) refuse_leases: bool,
    pub(crate) granted: Vec<(LeaseDeviceId, Vec<u32>)>,
    pub(crate) revoked: Vec<(LeaseDeviceId, NonZeroU32)>,
    next_lessee: u32,
    notes: Notes,
}

impl State {
    pub(crate) fn note(&self, line: impl Into<String>) {
        self.notes.note(line);
    }

    pub(crate) fn surface(&self, protocol_id: u32) -> SurfaceId {
        self.display
            .clients()
            .iter()
            .find_map(|client| client.object::<WlSurface>(protocol_id))
            .and_then(|surface| self.surfaces.lookup(&surface))
            .expect("the surface exists on the server")
    }

    pub(crate) fn output_of(&self, protocol_id: u32) -> Option<OutputId> {
        self.display
            .clients()
            .iter()
            .find_map(|client| client.object::<WlOutput>(protocol_id))
            .and_then(|output| self.outputs.output_of(&output))
    }
}

pub(crate) fn constraints(width: i32, height: i32) -> Constraints {
    Constraints {
        size: Size::new(width, height),
        formats: Rc::new(CaptureFormats {
            shm: [Fourcc::ARGB8888, Fourcc::XRGB8888].into(),
            dmabuf: None,
        }),
    }
}

impl SurfaceHost for State {
    fn surfaces(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }
}

impl OutputHost for State {
    fn outputs(&mut self) -> &mut Outputs {
        &mut self.outputs
    }

    fn output_bound(&mut self, output: OutputId, resource: &WlOutput) {
        self.bound_outputs.push(output);
        self.note(format!("bound output v{}", resource.version()));
    }
}

impl CaptureHost for State {
    type Toplevel = u32;

    fn captures(&mut self) -> &mut Captures<u32> {
        &mut self.captures
    }

    fn toplevel_source(&mut self, handle: &ExtForeignToplevelHandleV1) -> Option<u32> {
        handle.key::<u32>()
    }

    fn capture_session_started(
        &mut self,
        session: CaptureSessionId,
        info: SessionInfo<u32>,
    ) -> Option<Constraints> {
        self.started.push((session, info));
        self.offered.clone()
    }

    fn capture_requested(&mut self, request: CaptureRequest<u32>) {
        self.requests.push(request.clone());
        let Some(value) = self.copy_with else {
            return;
        };
        if let Some(BufferContent::Shm(mut shm)) = self.surfaces.buffer_content(&request.buffer) {
            shm.with_pixels_mut(|pixels| pixels.fill(value)).unwrap();
        }
        self.captures.ready(
            request.frame,
            FrameReady {
                presented: PRESENTED,
                transform: Transform::Rotate90,
                damage: Region::from(Rect::new(Point::new(0, 0), Size::new(2, 2))),
            },
        );
    }
}

impl BackgroundEffectHost for State {
    fn background_effects(&mut self) -> &mut BackgroundEffects {
        &mut self.effects
    }

    fn blur_region_committed(&mut self, surface: SurfaceId) {
        self.blur_commits.push(surface);
    }
}

impl DrmLeaseHost for State {
    fn drm_leases(&mut self) -> &mut DrmLeases {
        &mut self.leases
    }

    fn grant_lease(
        &mut self,
        device: LeaseDeviceId,
        connectors: &[u32],
    ) -> Result<LeaseGrant, LeaseRejected> {
        if self.refuse_leases {
            return Err(LeaseRejected::NoPipeline);
        }
        self.granted.push((device, connectors.to_vec()));
        self.next_lessee += 1;
        let fd = memfd_create("lease", MemfdFlags::CLOEXEC).map_err(std::io::Error::from)?;
        rustix::io::write(&fd, b"leased").map_err(std::io::Error::from)?;
        Ok(LeaseGrant {
            lessee: NonZeroU32::new(self.next_lessee).unwrap(),
            fd,
        })
    }

    fn revoke_lease(&mut self, device: LeaseDeviceId, lessee: NonZeroU32) {
        self.revoked.push((device, lessee));
    }
}

fn serve_test_globals(display: &DisplayHandle<State>) {
    display
        .serve::<WlSeat>(|_, _, _| {})
        .bound(|_, seat| seat.capabilities(wl_seat::Capability::POINTER));
    display.serve::<WlPointer>(|_, _, _| {});
    display.global::<WlSeat>().publish();
    display
        .serve::<ExtForeignToplevelListV1>(|_, list, request| {
            if let ext_foreign_toplevel_list_v1::Request::Stop = request {
                list.finished();
            }
        })
        .bound(|_, list| {
            let Some(client) = Client::from_resource(&list) else {
                return;
            };
            let handle = client.create::<ExtForeignToplevelHandleV1>(list.version());
            handle.set_key(TOPLEVEL);
            list.toplevel(&handle);
            handle.identifier("seven");
            handle.done();
        });
    display.serve::<ExtForeignToplevelHandleV1>(|_, _, _| {});
    display.global::<ExtForeignToplevelListV1>().publish();
}

impl Host for State {
    fn display_handle(&self) -> &DisplayHandle<Self> {
        &self.display
    }
}

pub(crate) struct Server(Display<State>);

impl Server {
    pub(crate) fn start() -> Self {
        Self::with(|_| {})
    }

    pub(crate) fn with(setup: impl FnOnce(&mut State) + Send + 'static) -> Self {
        let display = Display::start(move |reactor, notes| {
            let display = DisplayHandle::new(reactor);
            display.install(SurfaceProtocols);
            display.install(OutputProtocols);
            display.install(CaptureProtocols);
            display.install(BackgroundEffectProtocols);
            display.install(DrmLeaseProtocols);
            serve_test_globals(&display);
            let mut state = State {
                display,
                surfaces: Surfaces::new(),
                outputs: Outputs::new(),
                captures: Captures::new(),
                effects: BackgroundEffects::default(),
                leases: DrmLeases::new(),
                bound_outputs: Vec::new(),
                blur_commits: Vec::new(),
                started: Vec::new(),
                requests: Vec::new(),
                offered: Some(constraints(4, 2)),
                copy_with: None,
                refuse_leases: false,
                granted: Vec::new(),
                revoked: Vec::new(),
                next_lessee: 40,
                notes,
            };
            setup(&mut state);
            state
        });
        Self(display)
    }

    pub(crate) fn connect(&self) -> Wayland {
        self.connect_with(ClientOptions::new())
    }

    pub(crate) fn connect_unprivileged(&self) -> Wayland {
        self.connect_with(ClientOptions::new().unprivileged())
    }

    fn connect_with(&self, options: ClientOptions) -> Wayland {
        Wayland::new(self.0.connect(options))
    }
}

impl Deref for Server {
    type Target = Display<State>;

    fn deref(&self) -> &Display<State> {
        &self.0
    }
}
