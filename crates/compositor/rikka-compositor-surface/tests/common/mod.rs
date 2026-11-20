pub(crate) mod client;

use std::ops::Deref;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_server::{ClientOptions, DisplayHandle};
use rikka_compositor_surface::{
    Blocker, BlockerId, Committed, Dmabuf, SurfaceDamage, SurfaceHost, SurfaceId, SurfaceProtocols,
    Surfaces,
};
use rikka_compositor_types::OutputId;
pub(crate) use rikka_test_display::PATIENCE;
use rikka_test_display::{Display, Host, Notes};

pub(crate) use client::Wayland;

pub(crate) struct State {
    pub(crate) display: DisplayHandle<State>,
    pub(crate) surfaces: Surfaces,
    pub(crate) committed: Vec<Committed>,
    pub(crate) destroyed: Vec<SurfaceId>,
    pub(crate) refresh_wanted: Vec<OutputId>,
    pub(crate) reject_dmabufs: bool,
    notes: Notes,
    commit_signal: mpsc::Sender<()>,
}

#[derive(Debug)]
struct Gate;

impl Blocker for Gate {}

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

    pub(crate) fn gate(&mut self, protocol_id: u32) -> BlockerId {
        let surface = self.surface(protocol_id);
        self.surfaces.block(surface, |_| Gate).unwrap()
    }

    pub(crate) fn commits_of(&self, surface: SurfaceId) -> Vec<Committed> {
        self.committed
            .iter()
            .filter(|commit| commit.surface == surface)
            .copied()
            .collect()
    }

    pub(crate) fn take_damage(&mut self, surface: SurfaceId) -> SurfaceDamage {
        let mut damage = SurfaceDamage::default();
        self.surfaces.take_damage_into(surface, &mut damage);
        damage
    }
}

impl SurfaceHost for State {
    fn surfaces(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }

    fn committed(&mut self, commit: &Committed) {
        self.committed.push(*commit);
        let _ = self.commit_signal.send(());
    }

    fn destroyed(&mut self, surface: SurfaceId) {
        self.destroyed.push(surface);
    }

    fn import_dmabuf(&mut self, _dmabuf: &Dmabuf) -> bool {
        !self.reject_dmabufs
    }

    fn wants_refresh(&mut self, output: OutputId) {
        self.refresh_wanted.push(output);
    }

    fn output_of(
        &mut self,
        output: &rikka_compositor_protocols::wl_output::WlOutput,
    ) -> Option<OutputId> {
        output.key::<OutputId>()
    }
}

impl Host for State {
    fn display_handle(&self) -> &DisplayHandle<Self> {
        &self.display
    }
}

pub(crate) struct Server {
    display: Display<State>,
    commit_signals: Receiver<()>,
}

impl Server {
    pub(crate) fn start() -> Self {
        Self::with(|_| {})
    }

    pub(crate) fn with(setup: impl FnOnce(&mut State) + Send + 'static) -> Self {
        let (commit_signal, commit_signals) = mpsc::channel();
        let display = Display::start(move |reactor, notes| {
            let display = DisplayHandle::new(reactor);
            display.install(SurfaceProtocols);
            let mut state = State {
                display,
                surfaces: Surfaces::new(),
                committed: Vec::new(),
                destroyed: Vec::new(),
                refresh_wanted: Vec::new(),
                reject_dmabufs: false,
                notes,
                commit_signal,
            };
            setup(&mut state);
            state
        });
        Self {
            display,
            commit_signals,
        }
    }

    pub(crate) fn connect(&self) -> Wayland {
        Wayland::new(self.display.connect(ClientOptions::new()))
    }

    pub(crate) fn await_commits(&self, protocol_id: u32, count: usize) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            while self.commit_signals.try_recv().is_ok() {}
            let applied =
                self.call(move |state| state.commits_of(state.surface(protocol_id)).len());
            if applied >= count {
                return;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if self.commit_signals.recv_timeout(left).is_err() {
                panic!("surface {protocol_id} applied {applied} of {count} commits");
            }
        }
    }
}

impl Deref for Server {
    type Target = Display<State>;

    fn deref(&self) -> &Display<State> {
        &self.display
    }
}

pub(crate) fn serve_output(state: &mut State) {
    state
        .display
        .serve::<rikka_compositor_protocols::wl_output::WlOutput>(|_, _, _| {});
    state
        .display
        .global::<rikka_compositor_protocols::wl_output::WlOutput>()
        .version(4)
        .key(output(7))
        .publish();
}

pub(crate) fn output(raw: u64) -> OutputId {
    slotmap::KeyData::from_ffi(raw).into()
}
