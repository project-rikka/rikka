use std::ops::{Deref, DerefMut};
use std::path::Path;

use rikka_test_display::Canvas;
use rikka_wayland_client::Queue;
use rikka_wayland_client::wl_display::WlDisplay;
use rikka_wayland_client::{wl_buffer, wl_callback, wl_compositor, wl_output, wl_shm, wl_surface};
use rikka_wayland_client::{wp_presentation, wp_presentation_feedback};

use super::PATIENCE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Presented {
    pub(crate) tag: u32,
    pub(crate) sequence: u64,
    pub(crate) flags: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Recorded {
    pub(crate) frames: Vec<u32>,
    pub(crate) released: Vec<u32>,
    pub(crate) presented: Vec<Presented>,
    pub(crate) discarded: Vec<u32>,
    pub(crate) outputs: Vec<Described>,
    pub(crate) entered: Vec<u32>,
    pub(crate) synced: Vec<(u32, u32)>,
    pub(crate) captured: super::capture::Captured,
    pub(crate) shell: super::shell::ShellRecord,
    pub(crate) input: super::input::InputRecord,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Described {
    pub(crate) output: u32,
    pub(crate) make: String,
    pub(crate) model: String,
    pub(crate) mode: (i32, i32),
    pub(crate) done: bool,
}

fn on_wl_callback(
    recorded: &mut Recorded,
    proxy: wl_callback::WlCallback,
    event: wl_callback::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    if let wl_callback::Event::Done { .. } = event {
        recorded.frames.push(tag);
    }
}

fn on_wl_buffer(recorded: &mut Recorded, proxy: wl_buffer::WlBuffer, event: wl_buffer::Event) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    if let wl_buffer::Event::Release = event {
        recorded.released.push(tag);
    }
}

fn on_wp_presentation_feedback(
    recorded: &mut Recorded,
    proxy: wp_presentation_feedback::WpPresentationFeedback,
    event: wp_presentation_feedback::Event,
) {
    let Some(tag) = proxy.key::<u32>() else {
        return;
    };
    match event {
        wp_presentation_feedback::Event::Presented {
            seq_hi,
            seq_lo,
            flags,
            ..
        } => recorded.presented.push(Presented {
            tag,
            sequence: (u64::from(seq_hi) << 32) | u64::from(seq_lo),
            flags: flags.bits(),
        }),
        wp_presentation_feedback::Event::SyncOutput { output } => {
            recorded.synced.push((tag, output.id()));
        }
        wp_presentation_feedback::Event::Discarded => recorded.discarded.push(tag),
        _ => {}
    }
}

fn on_wl_surface(recorded: &mut Recorded, _: wl_surface::WlSurface, event: wl_surface::Event) {
    match event {
        wl_surface::Event::Enter { output } => recorded.entered.push(output.id()),
        wl_surface::Event::Leave { output } => {
            let id = output.id();
            recorded.entered.retain(|entered| *entered != id);
        }
        _ => {}
    }
}

fn on_wl_output(recorded: &mut Recorded, output: wl_output::WlOutput, event: wl_output::Event) {
    let id = output.id();
    let Some(described) = recorded.outputs.iter_mut().find(|entry| entry.output == id) else {
        return;
    };
    match event {
        wl_output::Event::Geometry { make, model, .. } => {
            described.make = make;
            described.model = model;
        }
        wl_output::Event::Mode {
            flags,
            width,
            height,
            ..
        } if flags.contains(wl_output::Mode::CURRENT) => described.mode = (width, height),
        wl_output::Event::Done => described.done = true,
        _ => {}
    }
}

pub(crate) struct Client {
    wayland: rikka_test_display::Client<Recorded>,
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    pub(crate) presentation: wp_presentation::WpPresentation,
}

impl Client {
    pub(crate) fn connect(socket: &Path) -> Self {
        let wayland = rikka_test_display::Client::connect_to(socket, |queue| {
            listen(queue);
            super::capture::listen(queue);
            super::shell::listen(queue);
            super::input::listen(queue);
        })
        .with_patience(PATIENCE);
        Self {
            compositor: wayland.global(6),
            shm: wayland.global(1),
            presentation: wayland.global(2),
            wayland,
        }
    }

    pub(crate) fn display(&self) -> rikka_wayland::Display {
        let display: WlDisplay = self.wayland.display();
        rikka_wayland::Display::from_connection(self.wayland.connection.clone(), display).unwrap()
    }

    pub(crate) fn surface(&self) -> wl_surface::WlSurface {
        self.compositor.create_surface()
    }

    pub(crate) fn output(&mut self) -> wl_output::WlOutput {
        let output: wl_output::WlOutput = self.wayland.global(4);
        self.wayland.received.outputs.push(Described {
            output: output.id(),
            ..Described::default()
        });
        output
    }

    pub(crate) fn pixels(&self, width: i32, height: i32, tag: u32) -> Canvas {
        let canvas = Canvas::new(&self.shm, width, height, wl_shm::Format::Argb8888);
        canvas.buffer.set_key(tag);
        canvas
    }
}

impl Deref for Client {
    type Target = rikka_test_display::Client<Recorded>;

    fn deref(&self) -> &Self::Target {
        &self.wayland
    }
}

impl DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.wayland
    }
}

fn listen(queue: &mut Queue<Recorded>) {
    queue.on::<wl_callback::WlCallback>(on_wl_callback);
    queue.on::<wl_buffer::WlBuffer>(on_wl_buffer);
    queue.on::<wp_presentation_feedback::WpPresentationFeedback>(on_wp_presentation_feedback);
    queue.on::<wl_surface::WlSurface>(on_wl_surface);
    queue.on::<wl_output::WlOutput>(on_wl_output);
}
