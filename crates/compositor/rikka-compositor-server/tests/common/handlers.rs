use rikka_compositor_protocols::wl_compositor::{self, WlCompositor};
use rikka_compositor_protocols::wl_data_device::{self, WlDataDevice};
use rikka_compositor_protocols::wl_data_device_manager::{self, WlDataDeviceManager};
use rikka_compositor_protocols::wl_data_offer::{self, WlDataOffer};
use rikka_compositor_protocols::wl_data_source::{self, WlDataSource};
use rikka_compositor_protocols::wl_output::{self, WlOutput};
use rikka_compositor_protocols::wl_region::{self, WlRegion};
use rikka_compositor_protocols::wl_seat::{self, WlSeat};
use rikka_compositor_protocols::wl_shm::{self, WlShm};
use rikka_compositor_protocols::wl_shm_pool::{self, WlShmPool};
use rikka_compositor_protocols::wl_subcompositor::{self, WlSubcompositor};
use rikka_compositor_protocols::wl_surface::{self, WlSurface};
use rikka_compositor_server::{Client, DisplayHandle, Key, Module};

use super::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tag(pub &'static str);

const TAGS: &[&str] = &["DP-1", "HDMI-A-1", "hotplugged", "clipboard"];

impl Key for Tag {
    fn into_raw(self) -> u64 {
        TAGS.iter()
            .position(|tag| *tag == self.0)
            .expect("tests only use known tags") as u64
    }

    fn from_raw(raw: u64) -> Self {
        Self(TAGS[raw as usize])
    }
}

fn compositor_bound(state: &mut State, compositor: WlCompositor) {
    state.note(format!("bound wl_compositor v{}", compositor.version()));
}

fn compositor_request(
    state: &mut State,
    _compositor: WlCompositor,
    request: wl_compositor::Request,
) {
    match request {
        wl_compositor::Request::CreateSurface { id } => {
            state.note(format!("created wl_surface#{}", id.id()));
        }
        wl_compositor::Request::CreateRegion { id } => {
            state.note(format!("created wl_region#{}", id.id()));
        }
        wl_compositor::Request::Release => {}
    }
}

fn compositor_destroyed(state: &mut State, _compositor: WlCompositor) {
    state.note("destroyed wl_compositor");
}

fn surface_request(state: &mut State, surface: WlSurface, request: wl_surface::Request) {
    if let wl_surface::Request::Destroy = request {
        state.note(format!(
            "request wl_surface#{} destroy while alive {}",
            surface.id(),
            surface.alive()
        ));
    }
}

fn surface_destroyed(state: &mut State, surface: WlSurface) {
    state.note(format!(
        "destroyed wl_surface#{} alive {}",
        surface.id(),
        surface.alive()
    ));
}

fn region_request(state: &mut State, region: WlRegion, request: wl_region::Request) {
    if let wl_region::Request::Add { .. } = request {
        let others: Vec<Client> = state
            .display
            .clients()
            .iter()
            .filter(|client| client.id() != region.client())
            .cloned()
            .collect();
        for other in &others {
            other.kill("evicted");
        }
        let global = state
            .display
            .global::<WlOutput>()
            .version(4)
            .key(Tag("hotplugged"))
            .publish();
        state.globals.insert("hotplugged", global);
        state.display.next_serial();
        state.note(format!("evicted {}", others.len()));
    }
}

fn region_destroyed(state: &mut State, region: WlRegion) {
    state.note(format!("destroyed wl_region#{}", region.id()));
}

fn output_bound(state: &mut State, output: WlOutput) {
    let tag = output.key::<Tag>().map_or("untagged", |tag| tag.0);
    state.note(format!("bound wl_output v{} {tag}", output.version()));
    output.name(tag);
    output.done();
}

fn output_request(state: &mut State, output: WlOutput, request: wl_output::Request) {
    match request {
        wl_output::Request::Release => {
            state.note(format!("released wl_output#{}", output.id()));
        }
    }
}

fn output_destroyed(state: &mut State, output: WlOutput) {
    state.note(format!("destroyed wl_output#{}", output.id()));
}

fn shm_bound(_state: &mut State, shm: WlShm) {
    shm.format(wl_shm::Format::Argb8888);
}

fn shm_request(state: &mut State, _shm: WlShm, request: wl_shm::Request) {
    if let wl_shm::Request::CreatePool { id, fd, size } = request {
        let mut byte = [0u8];
        let marker = rustix::io::pread(&fd, &mut byte, 0)
            .map(|_| byte[0])
            .unwrap_or(0);
        state.note(format!("pool#{} of {size} marked {marker}", id.id()));
    }
}

fn shm_pool_request(_state: &mut State, _pool: WlShmPool, _request: wl_shm_pool::Request) {}

fn seat_bound(_state: &mut State, seat: WlSeat) {
    seat.capabilities(wl_seat::Capability::empty());
}

fn seat_request(_state: &mut State, _seat: WlSeat, _request: wl_seat::Request) {}

fn data_device_manager_request(
    state: &mut State,
    _manager: WlDataDeviceManager,
    request: wl_data_device_manager::Request,
) {
    if let wl_data_device_manager::Request::GetDataDevice { id, .. } = request {
        let Some(client) = Client::from_resource(&id) else {
            return;
        };
        let offer: WlDataOffer = client.create(id.version());
        offer.set_key(Tag("clipboard"));
        id.data_offer(&offer);
        offer.offer("text/plain");
        id.selection(Some(&offer));
        state.note(format!("offered wl_data_offer#{:x}", offer.id()));
    }
}

fn data_device_request(
    _state: &mut State,
    _device: WlDataDevice,
    _request: wl_data_device::Request,
) {
}

fn data_source_request(
    _state: &mut State,
    _source: WlDataSource,
    _request: wl_data_source::Request<'_>,
) {
}

fn data_offer_request(state: &mut State, offer: WlDataOffer, request: wl_data_offer::Request<'_>) {
    if let wl_data_offer::Request::Receive { mime_type, .. } = request {
        let tag = offer.key::<Tag>().map_or("untagged", |tag| tag.0);
        state.note(format!("receive {mime_type} from {tag}"));
    }
}

fn data_offer_destroyed(state: &mut State, offer: WlDataOffer) {
    let tag = offer.key::<Tag>().map_or("untagged", |tag| tag.0);
    state.note(format!("destroyed wl_data_offer {tag}"));
}

pub(crate) fn serve(display: &DisplayHandle<State>) {
    display
        .serve::<WlCompositor>(compositor_request)
        .bound(compositor_bound)
        .destroyed(compositor_destroyed);
    display
        .serve::<WlSurface>(surface_request)
        .destroyed(surface_destroyed);
    display
        .serve::<WlRegion>(region_request)
        .destroyed(region_destroyed);
    display
        .serve::<WlOutput>(output_request)
        .bound(output_bound)
        .destroyed(output_destroyed);
    display.serve::<WlShm>(shm_request).bound(shm_bound);
    display.serve::<WlShmPool>(shm_pool_request);
    display.serve::<WlSeat>(seat_request).bound(seat_bound);
    display.serve::<WlDataDeviceManager>(data_device_manager_request);
    display.serve::<WlDataDevice>(data_device_request);
    display.serve::<WlDataSource>(data_source_request);
    display
        .serve::<WlDataOffer>(data_offer_request)
        .destroyed(data_offer_destroyed);
}

pub(crate) trait Counting {
    fn counter(&mut self) -> &mut u32;
}

impl Counting for State {
    fn counter(&mut self) -> &mut u32 {
        &mut self.counted
    }
}

pub(crate) struct Counter;

impl<S: Counting + 'static> Module<S> for Counter {
    fn install(self, display: &DisplayHandle<S>) {
        display
            .serve::<WlSubcompositor>(count)
            .bound(on_bind)
            .destroyed(on_destroy);
        display.global::<WlSubcompositor>().version(1).publish();
    }
}

fn count<S: Counting>(
    state: &mut S,
    _subcompositor: WlSubcompositor,
    request: wl_subcompositor::Request,
) {
    if let wl_subcompositor::Request::Destroy = request {
        *state.counter() += 1;
    }
}

fn on_bind<S: Counting>(state: &mut S, _subcompositor: WlSubcompositor) {
    *state.counter() += 10;
}

fn on_destroy<S: Counting>(state: &mut S, _subcompositor: WlSubcompositor) {
    *state.counter() += 100;
}
