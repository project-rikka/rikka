use std::collections::HashMap;
use std::io::Write;
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use rikka_nested::View;
use rikka_test_display::Canvas;
use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::time::Timespec;

use rikka_wayland_client::wp_cursor_shape_manager_v1;
use rikka_wayland_client::{Connection, Queue};
use rikka_wayland_client::{
    wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer,
    wl_data_source, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm,
    wl_subcompositor, wl_subsurface, wl_surface,
};
use rikka_wayland_client::{xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base};
use rikka_wayland_client::{zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1};
use rikka_wayland_client::{zwp_text_input_manager_v3, zwp_text_input_v3};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};

#[derive(Debug, Default)]
pub(crate) struct Received {
    pub(crate) globals: Vec<String>,
    pub(crate) configures: Vec<(i32, i32, Vec<u32>)>,
    pub(crate) serials: HashMap<u32, u32>,
    pub(crate) popups: Vec<(i32, i32, i32, i32)>,
    pub(crate) released: Vec<u32>,
    pub(crate) frames: u32,
    pub(crate) pointer: Vec<String>,
    pub(crate) keyboard: Vec<String>,
    pub(crate) text: Vec<String>,
    pub(crate) offers: HashMap<u32, Vec<String>>,
    pub(crate) selection: Option<u32>,
    pub(crate) data_offers: HashMap<u32, wl_data_offer::WlDataOffer>,
    pub(crate) closed: bool,
    pub(crate) entered: u32,
    pub(crate) pointer_serial: u32,
    pub(crate) dismissed: bool,
    pub(crate) drag: Vec<String>,
}

pub(crate) struct Client {
    connection: Connection,
    changes: Arc<OwnedFd>,
    queue: Queue<Received>,
    pub(crate) received: Received,
    pub(crate) compositor: wl_compositor::WlCompositor,
    pub(crate) subcompositor: wl_subcompositor::WlSubcompositor,
    pub(crate) shm: wl_shm::WlShm,
    pub(crate) wm_base: xdg_wm_base::XdgWmBase,
    pub(crate) seat: wl_seat::WlSeat,
    pub(crate) data_devices: wl_data_device_manager::WlDataDeviceManager,
    pub(crate) text_inputs: zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    pub(crate) dmabuf: Option<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1>,
    pub(crate) cursor_shapes: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
    canvases: Vec<Canvas>,
    _memory: Vec<OwnedFd>,
}

pub(crate) struct Window {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) xdg: xdg_surface::XdgSurface,
    pub(crate) toplevel: xdg_toplevel::XdgToplevel,
}

pub(crate) struct Popup {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) xdg: xdg_surface::XdgSurface,
    pub(crate) _popup: xdg_popup::XdgPopup,
}

impl Client {
    pub(crate) fn connect(path: &Path) -> Client {
        let (connection, display) =
            rikka_wayland_client::connect_to(path).expect("the nested socket accepts");
        let mut queue = connection.queue();
        listen(&mut queue);
        let registry = display.via(&queue.route()).get_registry();
        let mut received = Received::default();
        let mut names = HashMap::new();
        queue
            .roundtrip(&mut received)
            .expect("the registry answers");
        for global in &received.globals {
            let mut parts = global.split(' ');
            let interface = parts.next().unwrap_or_default().to_owned();
            let name: u32 = parts
                .next()
                .and_then(|name| name.parse().ok())
                .unwrap_or_default();
            let version: u32 = parts
                .next()
                .and_then(|version| version.parse().ok())
                .unwrap_or(1);
            names.insert(interface, (name, version));
        }
        let bind = |interface: &str| names.get(interface).copied();
        let compositor = {
            let (name, version) = bind("wl_compositor").expect("wl_compositor");
            registry.bind(name, version.min(6))
        };
        let subcompositor = {
            let (name, _) = bind("wl_subcompositor").expect("wl_subcompositor");
            registry.bind(name, 1)
        };
        let shm = {
            let (name, _) = bind("wl_shm").expect("wl_shm");
            registry.bind(name, 1)
        };
        let wm_base = {
            let (name, version) = bind("xdg_wm_base").expect("xdg_wm_base");
            registry.bind(name, version.min(6))
        };
        let seat = {
            let (name, version) = bind("wl_seat").expect("wl_seat");
            registry.bind(name, version.min(9))
        };
        let data_devices = {
            let (name, version) = bind("wl_data_device_manager").expect("data devices");
            registry.bind(name, version.min(3))
        };
        let text_inputs = {
            let (name, _) = bind("zwp_text_input_manager_v3").expect("text input");
            registry.bind(name, 1)
        };
        let dmabuf =
            bind("zwp_linux_dmabuf_v1").map(|(name, version)| registry.bind(name, version.min(4)));
        let cursor_shapes = {
            let (name, _) = bind("wp_cursor_shape_manager_v1").expect("cursor shapes");
            registry.bind(name, 1)
        };
        if let Some((name, _)) = bind("wl_output") {
            let _: wl_output::WlOutput = registry.bind(name, 4);
        }
        let mut client = Client {
            connection,
            changes: Arc::new(
                eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK).expect("an eventfd"),
            ),
            queue,
            received,
            compositor,
            subcompositor,
            shm,
            wm_base,
            seat,
            data_devices,
            text_inputs,
            dmabuf,
            cursor_shapes,
            canvases: Vec::new(),
            _memory: Vec::new(),
        };
        client.roundtrip();
        client
    }

    pub(crate) fn roundtrip(&mut self) {
        self.queue
            .roundtrip(&mut self.received)
            .expect("the nested display answers");
    }

    pub(crate) fn notifier(&self) -> impl Fn() + Send + Sync + 'static {
        let changes = Arc::clone(&self.changes);
        move || {
            let _ = rustix::io::write(&*changes, &1u64.to_ne_bytes());
        }
    }

    pub(crate) fn follow(&self, view: &View) {
        view.watch(self.notifier());
    }

    pub(crate) fn wait(&mut self, deadline: Instant) -> bool {
        let left = deadline.saturating_duration_since(Instant::now());
        let timeout = Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds = [
            PollFd::new(&self.connection, PollFlags::IN),
            PollFd::new(&*self.changes, PollFlags::IN),
        ];
        let ready = poll(&mut fds, Some(&timeout)).expect("polling the client");
        let mut count = [0u8; 8];
        let _ = rustix::io::read(&*self.changes, &mut count);
        ready > 0
    }

    pub(crate) fn barrier(&mut self) {
        self.roundtrip();
        self.roundtrip();
    }

    pub(crate) fn alive(&mut self) -> bool {
        self.queue.roundtrip(&mut self.received).is_ok()
    }

    pub(crate) fn advertises(&self, interface: &str) -> bool {
        self.received
            .globals
            .iter()
            .any(|global| global.split(' ').next() == Some(interface))
    }

    pub(crate) fn window(&mut self, app_id: Option<&str>) -> Window {
        let surface = self.compositor.create_surface();
        let xdg = self.wm_base.get_xdg_surface(&surface);
        let toplevel = xdg.get_toplevel();
        if let Some(app_id) = app_id {
            toplevel.set_app_id(app_id);
        }
        surface.commit();
        self.roundtrip();
        Window {
            surface,
            xdg,
            toplevel,
        }
    }

    pub(crate) fn ack(&mut self, xdg: &xdg_surface::XdgSurface) {
        if let Some(serial) = self.received.serials.get(&xdg.id()) {
            xdg.ack_configure(*serial);
        }
    }

    pub(crate) fn shm_buffer(&mut self, width: i32, height: i32, argb: u32) -> wl_buffer::WlBuffer {
        let canvas = Canvas::new(&self.shm, width, height, wl_shm::Format::Argb8888);
        canvas.fill(0..height, argb.to_le_bytes());
        let buffer = canvas.buffer.clone();
        self.canvases.push(canvas);
        buffer
    }

    pub(crate) fn dmabuf_buffer(&mut self, width: i32, height: i32) -> wl_buffer::WlBuffer {
        let dmabuf = self
            .dmabuf
            .clone()
            .expect("the nested display offers dmabufs");
        let stride = (width * 4) as u32;
        let memory = memfd_create("nested-dmabuf", MemfdFlags::CLOEXEC).unwrap();
        ftruncate(&memory, u64::from(stride) * height as u64).unwrap();
        let params = dmabuf.create_params();
        params.add(memory.as_fd(), 0, 0, stride, 0, 0);
        let buffer = params.create_immed(
            width,
            height,
            u32::from_le_bytes(*b"XR24"),
            zwp_linux_buffer_params_v1::Flags::empty(),
        );
        self._memory.push(memory);
        buffer
    }

    pub(crate) fn show(&mut self, surface: &wl_surface::WlSurface, buffer: &wl_buffer::WlBuffer) {
        surface.attach(Some(buffer), 0, 0);
        surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
        surface.frame();
        surface.commit();
        self.roundtrip();
    }

    pub(crate) fn map(&mut self, window: &Window, width: i32, height: i32) -> wl_buffer::WlBuffer {
        self.ack(&window.xdg);
        let buffer = self.shm_buffer(width, height, 0xff20_4060);
        self.show(&window.surface, &buffer);
        buffer
    }

    pub(crate) fn subsurface(
        &mut self,
        parent: &wl_surface::WlSurface,
        x: i32,
        y: i32,
    ) -> (wl_surface::WlSurface, wl_subsurface::WlSubsurface) {
        let surface = self.compositor.create_surface();
        let subsurface = self.subcompositor.get_subsurface(&surface, parent);
        subsurface.set_position(x, y);
        subsurface.set_desync();
        (surface, subsurface)
    }

    pub(crate) fn popup(
        &mut self,
        parent: &xdg_surface::XdgSurface,
        anchor: (i32, i32),
        size: (i32, i32),
        grab: Option<u32>,
    ) -> Popup {
        let positioner = self.wm_base.create_positioner();
        positioner.set_size(size.0, size.1);
        positioner.set_anchor_rect(anchor.0, anchor.1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::TopLeft);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        let surface = self.compositor.create_surface();
        let xdg = self.wm_base.get_xdg_surface(&surface);
        let popup = xdg.get_popup(Some(parent), &positioner);
        if let Some(serial) = grab {
            popup.grab(&self.seat, serial);
        }
        surface.commit();
        self.roundtrip();
        Popup {
            surface,
            xdg,
            _popup: popup,
        }
    }

    pub(crate) fn pointer(&mut self) -> wl_pointer::WlPointer {
        let pointer = self.seat.get_pointer();
        self.roundtrip();
        pointer
    }

    pub(crate) fn keyboard(&mut self) -> wl_keyboard::WlKeyboard {
        let keyboard = self.seat.get_keyboard();
        self.roundtrip();
        keyboard
    }

    pub(crate) fn text_input(&mut self) -> zwp_text_input_v3::ZwpTextInputV3 {
        let text_input = self.text_inputs.get_text_input(&self.seat);
        self.roundtrip();
        text_input
    }

    pub(crate) fn data_device(&mut self) -> wl_data_device::WlDataDevice {
        let device = self.data_devices.get_data_device(&self.seat);
        self.roundtrip();
        device
    }

    pub(crate) fn source(&mut self, mimes: &[&str]) -> wl_data_source::WlDataSource {
        let source = self.data_devices.create_data_source();
        for mime in mimes {
            source.offer(mime);
        }
        source
    }
}

fn on_wl_registry(received: &mut Received, _: wl_registry::WlRegistry, event: wl_registry::Event) {
    if let wl_registry::Event::Global {
        name,
        interface,
        version,
    } = event
    {
        received
            .globals
            .push(format!("{interface} {name} {version}"));
    }
}

fn on_xdg_wm_base(_: &mut Received, wm_base: xdg_wm_base::XdgWmBase, event: xdg_wm_base::Event) {
    if let xdg_wm_base::Event::Ping { serial } = event {
        wm_base.pong(serial);
    }
}

fn on_xdg_surface(
    received: &mut Received,
    xdg: xdg_surface::XdgSurface,
    event: xdg_surface::Event,
) {
    if let xdg_surface::Event::Configure { serial } = event {
        received.serials.insert(xdg.id(), serial);
    }
}

fn on_xdg_toplevel(
    received: &mut Received,
    _: xdg_toplevel::XdgToplevel,
    event: xdg_toplevel::Event,
) {
    match event {
        xdg_toplevel::Event::Configure {
            width,
            height,
            states,
        } => {
            let states = states
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| u32::from_ne_bytes(*chunk))
                .collect();
            received.configures.push((width, height, states));
        }
        xdg_toplevel::Event::Close => received.closed = true,
        _ => {}
    }
}

fn on_xdg_popup(received: &mut Received, _: xdg_popup::XdgPopup, event: xdg_popup::Event) {
    match event {
        xdg_popup::Event::Configure {
            x,
            y,
            width,
            height,
        } => received.popups.push((x, y, width, height)),
        xdg_popup::Event::PopupDone => received.dismissed = true,
        _ => {}
    }
}

fn on_wl_buffer(received: &mut Received, buffer: wl_buffer::WlBuffer, event: wl_buffer::Event) {
    if let wl_buffer::Event::Release = event {
        received.released.push(buffer.id());
    }
}

fn on_wl_callback(received: &mut Received, _: wl_callback::WlCallback, event: wl_callback::Event) {
    if let wl_callback::Event::Done { .. } = event {
        received.frames += 1;
    }
}

fn on_wl_pointer(received: &mut Received, _: wl_pointer::WlPointer, event: wl_pointer::Event) {
    let line = match event {
        wl_pointer::Event::Enter {
            surface,
            surface_x,
            surface_y,
            serial,
        } => {
            received.pointer_serial = serial;
            format!("enter {} {surface_x} {surface_y}", surface.id())
        }
        wl_pointer::Event::Leave { surface, .. } => {
            format!("leave {}", surface.id())
        }
        wl_pointer::Event::Motion {
            surface_x,
            surface_y,
            ..
        } => format!("motion {surface_x} {surface_y}"),
        wl_pointer::Event::Button {
            button,
            state,
            serial,
            ..
        } => {
            received.pointer_serial = serial;
            let pressed = matches!(state, Ok(wl_pointer::ButtonState::Pressed));
            format!("button {button:#x} {pressed}")
        }
        wl_pointer::Event::Axis { axis, value, .. } => format!("axis {axis:?} {value}"),
        wl_pointer::Event::AxisSource { axis_source } => format!("source {axis_source:?}"),
        wl_pointer::Event::AxisStop { axis, .. } => format!("stop {axis:?}"),
        wl_pointer::Event::AxisValue120 { axis, value120 } => {
            format!("value120 {axis:?} {value120}")
        }
        wl_pointer::Event::AxisDiscrete { axis, discrete } => {
            format!("discrete {axis:?} {discrete}")
        }
        wl_pointer::Event::Frame => "frame".to_owned(),
        other => format!("{other:?}"),
    };
    received.pointer.push(line);
}

fn on_wl_keyboard(received: &mut Received, _: wl_keyboard::WlKeyboard, event: wl_keyboard::Event) {
    let line = match event {
        wl_keyboard::Event::Keymap { .. } => "keymap".to_owned(),
        wl_keyboard::Event::Enter {
            surface, serial, ..
        } => {
            received.entered = serial;
            format!("enter {}", surface.id())
        }
        wl_keyboard::Event::Leave { surface, .. } => {
            format!("leave {}", surface.id())
        }
        wl_keyboard::Event::Key { key, state, .. } => {
            let pressed = matches!(state, Ok(wl_keyboard::KeyState::Pressed));
            format!("key {key} {pressed}")
        }
        wl_keyboard::Event::Modifiers { mods_depressed, .. } => {
            format!("modifiers {mods_depressed}")
        }
        wl_keyboard::Event::RepeatInfo { rate, delay } => format!("repeat {rate} {delay}"),
        other => format!("{other:?}"),
    };
    received.keyboard.push(line);
}

fn on_zwp_text_input_v3(
    received: &mut Received,
    _: zwp_text_input_v3::ZwpTextInputV3,
    event: zwp_text_input_v3::Event,
) {
    let line = match event {
        zwp_text_input_v3::Event::Enter { .. } => "enter".to_owned(),
        zwp_text_input_v3::Event::Leave { .. } => "leave".to_owned(),
        zwp_text_input_v3::Event::PreeditString {
            text,
            cursor_begin,
            cursor_end,
        } => format!(
            "preedit {} {cursor_begin} {cursor_end}",
            text.unwrap_or_default()
        ),
        zwp_text_input_v3::Event::CommitString { text } => {
            format!("commit {}", text.unwrap_or_default())
        }
        zwp_text_input_v3::Event::DeleteSurroundingText {
            before_length,
            after_length,
        } => format!("delete {before_length} {after_length}"),
        zwp_text_input_v3::Event::Done { serial } => format!("done {serial}"),
        other => format!("{other:?}"),
    };
    received.text.push(line);
}

fn on_wl_data_device(
    received: &mut Received,
    _: wl_data_device::WlDataDevice,
    event: wl_data_device::Event,
) {
    match event {
        wl_data_device::Event::DataOffer { id } => {
            let key = id.id();
            received.offers.insert(key, Vec::new());
            received.data_offers.insert(key, id);
        }
        wl_data_device::Event::Selection { id } => {
            received.selection = id.map(|offer| offer.id());
        }
        wl_data_device::Event::Enter { serial, id, .. } => {
            let offer = id.map_or(0, |offer| offer.id());
            received.drag.push(format!("enter {offer} {serial}"));
        }
        wl_data_device::Event::Leave => received.drag.push("leave".to_owned()),
        wl_data_device::Event::Drop => received.drag.push("drop".to_owned()),
        _ => {}
    }
}

fn on_wl_data_offer(
    received: &mut Received,
    offer: wl_data_offer::WlDataOffer,
    event: wl_data_offer::Event,
) {
    if let wl_data_offer::Event::Offer { mime_type } = event {
        received
            .offers
            .entry(offer.id())
            .or_default()
            .push(mime_type);
    }
}

fn on_wl_data_source(
    _: &mut Received,
    _: wl_data_source::WlDataSource,
    event: wl_data_source::Event,
) {
    if let wl_data_source::Event::Send { mime_type, fd } = event {
        let _ = std::fs::File::from(fd).write_all(format!("nested {mime_type}").as_bytes());
    }
}

fn listen(queue: &mut Queue<Received>) {
    queue.on::<wl_registry::WlRegistry>(on_wl_registry);
    queue.on::<xdg_wm_base::XdgWmBase>(on_xdg_wm_base);
    queue.on::<xdg_surface::XdgSurface>(on_xdg_surface);
    queue.on::<xdg_toplevel::XdgToplevel>(on_xdg_toplevel);
    queue.on::<xdg_popup::XdgPopup>(on_xdg_popup);
    queue.on::<wl_buffer::WlBuffer>(on_wl_buffer);
    queue.on::<wl_callback::WlCallback>(on_wl_callback);
    queue.on::<wl_pointer::WlPointer>(on_wl_pointer);
    queue.on::<wl_keyboard::WlKeyboard>(on_wl_keyboard);
    queue.on::<zwp_text_input_v3::ZwpTextInputV3>(on_zwp_text_input_v3);
    queue.on::<wl_data_device::WlDataDevice>(on_wl_data_device);
    queue.on::<wl_data_offer::WlDataOffer>(on_wl_data_offer);
    queue.on::<wl_data_source::WlDataSource>(on_wl_data_source);
}
