use std::collections::HashMap;
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use rikka_compositor_server::ClientId;
use rikka_test_display::{Canvas, Client};
use rikka_wayland_client::{Key, Proxy};
use rikka_wayland_client::{
    Queue, ext_data_control_device_v1, ext_data_control_offer_v1, ext_data_control_source_v1,
    wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer, wl_data_source,
    wl_keyboard, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch,
    zwp_primary_selection_device_v1, zwp_primary_selection_offer_v1,
    zwp_primary_selection_source_v1,
};
use rustix::pipe::{PipeFlags, pipe_with};
use std::sync::Mutex;

#[derive(Debug, Default)]
pub(crate) struct Received {
    pub(crate) keyboard_enters: Vec<u32>,
    pub(crate) touch: Vec<String>,
    pub(crate) device: Vec<String>,
    pub(crate) primary: Vec<String>,
    pub(crate) control: Vec<String>,
    pub(crate) mime_types: HashMap<u32, Vec<String>>,
    pub(crate) offer_log: Vec<String>,
    pub(crate) data_source: Vec<String>,
    pub(crate) primary_source: Vec<String>,
    pub(crate) control_source: Vec<String>,
    pub(crate) data_offers: HashMap<u32, wl_data_offer::WlDataOffer>,
    pub(crate) primary_offers:
        HashMap<u32, zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1>,
    pub(crate) control_offers: HashMap<u32, ext_data_control_offer_v1::ExtDataControlOfferV1>,
}

impl Received {
    pub(crate) fn last_enter_serial(&self) -> u32 {
        *self.keyboard_enters.last().expect("the keyboard entered")
    }

    pub(crate) fn offered(&self, id: u32) -> Vec<String> {
        self.mime_types.get(&id).cloned().unwrap_or_default()
    }

    pub(crate) fn take_device(&mut self) -> Vec<String> {
        std::mem::take(&mut self.device)
    }
}

static PAYLOADS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Payload(u32);

impl Payload {
    pub(crate) fn new(text: &str) -> Self {
        let mut payloads = PAYLOADS.lock().unwrap();
        payloads.push(text.to_owned());
        Self(payloads.len() as u32 - 1)
    }

    fn text(self) -> String {
        PAYLOADS.lock().unwrap()[self.0 as usize].clone()
    }
}

impl Key for Payload {
    fn into_raw(self) -> u64 {
        u64::from(self.0)
    }

    fn from_raw(raw: u64) -> Self {
        Self(raw as u32)
    }
}

fn id_of(proxy: Option<&impl Proxy>) -> Option<u32> {
    proxy.map(|proxy| proxy.id())
}

fn bits(value: wl_data_device_manager::DndAction) -> u32 {
    value.bits()
}

fn write_payload(fd: OwnedFd, payload: &str) {
    let mut file = std::fs::File::from(fd);
    let _ = file.write_all(payload.as_bytes());
}

fn on_wl_keyboard(received: &mut Received, _: wl_keyboard::WlKeyboard, event: wl_keyboard::Event) {
    if let wl_keyboard::Event::Enter { serial, .. } = event {
        received.keyboard_enters.push(serial);
    }
}

fn on_wl_data_device(
    received: &mut Received,
    _: wl_data_device::WlDataDevice,
    event: wl_data_device::Event,
) {
    let line = match event {
        wl_data_device::Event::DataOffer { id } => {
            let key = id.id();
            received.mime_types.insert(key, Vec::new());
            received.data_offers.insert(key, id);
            format!("data_offer {key}")
        }
        wl_data_device::Event::Enter {
            surface, x, y, id, ..
        } => format!("enter {} {x} {y} {:?}", surface.id(), id_of(id.as_ref())),
        wl_data_device::Event::Leave => "leave".to_owned(),
        wl_data_device::Event::Motion { x, y, .. } => format!("motion {x} {y}"),
        wl_data_device::Event::Drop => "drop".to_owned(),
        wl_data_device::Event::Selection { id } => {
            format!("selection {:?}", id_of(id.as_ref()))
        }
        _ => "unknown".to_owned(),
    };
    received.device.push(line);
}

fn on_wl_data_offer(
    received: &mut Received,
    offer: wl_data_offer::WlDataOffer,
    event: wl_data_offer::Event,
) {
    let key = offer.id();
    match event {
        wl_data_offer::Event::Offer { mime_type } => {
            received.mime_types.entry(key).or_default().push(mime_type);
        }
        wl_data_offer::Event::SourceActions { source_actions } => {
            received
                .offer_log
                .push(format!("source_actions {}", bits(source_actions)));
        }
        wl_data_offer::Event::Action { dnd_action } => {
            received
                .offer_log
                .push(format!("action {}", bits(dnd_action)));
        }
        _ => {}
    }
}

fn on_wl_data_source(
    received: &mut Received,
    proxy: wl_data_source::WlDataSource,
    event: wl_data_source::Event,
) {
    let payload = &proxy.key::<Payload>().expect("the object is tagged").text();
    let line = match event {
        wl_data_source::Event::Send { mime_type, fd } => {
            write_payload(fd, payload);
            format!("send {mime_type}")
        }
        wl_data_source::Event::Target { mime_type } => format!("target {mime_type:?}"),
        wl_data_source::Event::Cancelled => "cancelled".to_owned(),
        wl_data_source::Event::DndDropPerformed => "dnd_drop_performed".to_owned(),
        wl_data_source::Event::DndFinished => "dnd_finished".to_owned(),
        wl_data_source::Event::Action { dnd_action } => format!("action {}", bits(dnd_action)),
        _ => "unknown".to_owned(),
    };
    received.data_source.push(line);
}

fn on_zwp_primary_selection_device_v1(
    received: &mut Received,
    _: zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1,
    event: zwp_primary_selection_device_v1::Event,
) {
    let line = match event {
        zwp_primary_selection_device_v1::Event::DataOffer { offer } => {
            let key = offer.id();
            received.mime_types.insert(key, Vec::new());
            received.primary_offers.insert(key, offer);
            format!("data_offer {key}")
        }
        zwp_primary_selection_device_v1::Event::Selection { id } => {
            format!("selection {:?}", id_of(id.as_ref()))
        }
        _ => "unknown".to_owned(),
    };
    received.primary.push(line);
}

fn on_zwp_primary_selection_offer_v1(
    received: &mut Received,
    offer: zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1,
    event: zwp_primary_selection_offer_v1::Event,
) {
    if let zwp_primary_selection_offer_v1::Event::Offer { mime_type } = event {
        received
            .mime_types
            .entry(offer.id())
            .or_default()
            .push(mime_type);
    }
}

fn on_zwp_primary_selection_source_v1(
    received: &mut Received,
    proxy: zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
    event: zwp_primary_selection_source_v1::Event,
) {
    let payload = &proxy.key::<Payload>().expect("the object is tagged").text();
    let line = match event {
        zwp_primary_selection_source_v1::Event::Send { mime_type, fd } => {
            write_payload(fd, payload);
            format!("send {mime_type}")
        }
        zwp_primary_selection_source_v1::Event::Cancelled => "cancelled".to_owned(),
        _ => "unknown".to_owned(),
    };
    received.primary_source.push(line);
}

fn on_ext_data_control_device_v1(
    received: &mut Received,
    _: ext_data_control_device_v1::ExtDataControlDeviceV1,
    event: ext_data_control_device_v1::Event,
) {
    let line = match event {
        ext_data_control_device_v1::Event::DataOffer { id } => {
            let key = id.id();
            received.mime_types.insert(key, Vec::new());
            received.control_offers.insert(key, id);
            format!("data_offer {key}")
        }
        ext_data_control_device_v1::Event::Selection { id } => {
            format!("selection {:?}", id_of(id.as_ref()))
        }
        ext_data_control_device_v1::Event::PrimarySelection { id } => {
            format!("primary_selection {:?}", id_of(id.as_ref()))
        }
        ext_data_control_device_v1::Event::Finished => "finished".to_owned(),
        _ => "unknown".to_owned(),
    };
    received.control.push(line);
}

fn on_ext_data_control_offer_v1(
    received: &mut Received,
    offer: ext_data_control_offer_v1::ExtDataControlOfferV1,
    event: ext_data_control_offer_v1::Event,
) {
    if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
        received
            .mime_types
            .entry(offer.id())
            .or_default()
            .push(mime_type);
    }
}

fn on_ext_data_control_source_v1(
    received: &mut Received,
    proxy: ext_data_control_source_v1::ExtDataControlSourceV1,
    event: ext_data_control_source_v1::Event,
) {
    let payload = &proxy.key::<Payload>().expect("the object is tagged").text();
    let line = match event {
        ext_data_control_source_v1::Event::Send { mime_type, fd } => {
            write_payload(fd, payload);
            format!("send {mime_type}")
        }
        ext_data_control_source_v1::Event::Cancelled => "cancelled".to_owned(),
        _ => "unknown".to_owned(),
    };
    received.control_source.push(line);
}

pub(crate) fn pipe() -> (OwnedFd, OwnedFd) {
    pipe_with(PipeFlags::CLOEXEC).unwrap()
}

pub(crate) fn drain(fd: OwnedFd) -> String {
    let mut text = String::new();
    std::fs::File::from(fd).read_to_string(&mut text).unwrap();
    text
}

pub(crate) struct Mapped {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) id: u32,
    _canvas: Canvas,
}

pub(crate) struct Wayland {
    inner: Client<Received>,
    pub(crate) client: ClientId,
}

impl Deref for Wayland {
    type Target = Client<Received>;

    fn deref(&self) -> &Client<Received> {
        &self.inner
    }
}

impl DerefMut for Wayland {
    fn deref_mut(&mut self) -> &mut Client<Received> {
        &mut self.inner
    }
}

fn listen(queue: &mut Queue<Received>) {
    queue.on::<wl_keyboard::WlKeyboard>(on_wl_keyboard);
    queue.on::<wl_data_device::WlDataDevice>(on_wl_data_device);
    queue.on::<wl_data_offer::WlDataOffer>(on_wl_data_offer);
    queue.on::<wl_data_source::WlDataSource>(on_wl_data_source);
    queue.on::<zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1>(
        on_zwp_primary_selection_device_v1,
    );
    queue.on::<zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1>(
        on_zwp_primary_selection_offer_v1,
    );
    queue.on::<zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1>(
        on_zwp_primary_selection_source_v1,
    );
    queue.on::<ext_data_control_device_v1::ExtDataControlDeviceV1>(on_ext_data_control_device_v1);
    queue.on::<ext_data_control_offer_v1::ExtDataControlOfferV1>(on_ext_data_control_offer_v1);
    queue.on::<ext_data_control_source_v1::ExtDataControlSourceV1>(on_ext_data_control_source_v1);
    queue.on::<wl_touch::WlTouch>(on_wl_touch);
}

impl Wayland {
    pub(crate) fn new(stream: UnixStream, client: ClientId) -> Self {
        Self {
            inner: Client::connect(stream, listen),
            client,
        }
    }

    pub(crate) fn seat(&self) -> wl_seat::WlSeat {
        self.global(10)
    }

    pub(crate) fn data_device(
        &self,
        seat: &wl_seat::WlSeat,
        version: u32,
    ) -> (
        wl_data_device_manager::WlDataDeviceManager,
        wl_data_device::WlDataDevice,
    ) {
        let manager: wl_data_device_manager::WlDataDeviceManager = self.global(version);
        let device = manager.get_data_device(seat);
        (manager, device)
    }

    pub(crate) fn data_source(
        &self,
        manager: &wl_data_device_manager::WlDataDeviceManager,
        payload: &str,
        mime_types: &[&str],
    ) -> wl_data_source::WlDataSource {
        let source = manager.create_data_source().with_key(Payload::new(payload));
        for mime_type in mime_types {
            source.offer(mime_type);
        }
        source
    }

    pub(crate) fn data_offer(&self, id: u32) -> wl_data_offer::WlDataOffer {
        self.received.data_offers[&id].clone()
    }

    pub(crate) fn mapped(&mut self, width: i32, height: i32) -> Mapped {
        let compositor: wl_compositor::WlCompositor = self.global(6);
        let surface = compositor.create_surface();
        let canvas = self.canvas(width, height, wl_shm::Format::Argb8888);
        surface.attach(Some(&canvas.buffer), 0, 0);
        surface.commit();
        self.roundtrip();
        let id = surface.id();
        Mapped {
            surface,
            id,
            _canvas: canvas,
        }
    }

    pub(crate) fn surface(&self) -> (wl_surface::WlSurface, u32) {
        let compositor: wl_compositor::WlCompositor = self.global(6);
        let surface = compositor.create_surface();
        let id = surface.id();
        (surface, id)
    }
}

pub(crate) struct Participant {
    pub(crate) seat: wl_seat::WlSeat,
    _keyboard: wl_keyboard::WlKeyboard,
    _pointer: wl_pointer::WlPointer,
    pub(crate) manager: wl_data_device_manager::WlDataDeviceManager,
    pub(crate) device: wl_data_device::WlDataDevice,
    pub(crate) window: Mapped,
}

impl Wayland {
    pub(crate) fn participant(&mut self) -> Participant {
        self.participant_at(3)
    }

    pub(crate) fn participant_at(&mut self, version: u32) -> Participant {
        let seat = self.seat();
        let keyboard = seat.get_keyboard();
        let pointer = seat.get_pointer();
        let (manager, device) = self.data_device(&seat, version);
        let window = self.mapped(50, 50);
        Participant {
            seat,
            _keyboard: keyboard,
            _pointer: pointer,
            manager,
            device,
            window,
        }
    }
}

fn on_wl_touch(received: &mut Received, _: wl_touch::WlTouch, event: wl_touch::Event) {
    let line = match event {
        wl_touch::Event::Down { serial, id, .. } => format!("down {id} {serial}"),
        wl_touch::Event::Up { id, .. } => format!("up {id}"),
        wl_touch::Event::Motion { id, .. } => format!("motion {id}"),
        wl_touch::Event::Frame => "frame".to_owned(),
        wl_touch::Event::Cancel => "cancel".to_owned(),
        _ => return,
    };
    received.touch.push(line);
}

impl Received {
    pub(crate) fn touch_down_serial(&self) -> u32 {
        self.touch
            .iter()
            .rev()
            .find_map(|line| line.strip_prefix("down "))
            .and_then(|rest| rest.split(' ').nth(1))
            .expect("a touch went down")
            .parse()
            .unwrap()
    }
}
