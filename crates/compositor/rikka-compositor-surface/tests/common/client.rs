mod extensions;

use std::ops::{Deref, DerefMut};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use rikka_test_display::Client;
use rikka_wayland_client::wp_fractional_scale_v1;
use rikka_wayland_client::{Key, Queue, raw_value};
use rikka_wayland_client::{wl_buffer, wl_callback, wl_compositor, wl_shm, wl_surface};
use rikka_wayland_client::{wp_presentation, wp_presentation_feedback};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Callback {
    Frame(u32),
}

impl Key for Callback {
    fn into_raw(self) -> u64 {
        let Self::Frame(tag) = self;
        u64::from(tag)
    }

    fn from_raw(raw: u64) -> Self {
        Self::Frame(raw as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Presented {
    pub(crate) tag: u32,
    pub(crate) seconds: u64,
    pub(crate) nanoseconds: u32,
    pub(crate) refresh: u32,
    pub(crate) sequence: u64,
    pub(crate) flags: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Received {
    pub(crate) callbacks: Vec<(Callback, u32)>,
    pub(crate) released: Vec<u32>,
    pub(crate) presented: Vec<Presented>,
    pub(crate) discarded: Vec<u32>,
    pub(crate) fractional: Vec<u32>,
    pub(crate) buffer_scales: Vec<i32>,
    pub(crate) buffer_transforms: Vec<u32>,
    pub(crate) formats: Vec<u32>,
    pub(crate) clock: Option<u32>,
    pub(crate) entered: Vec<u32>,
    pub(crate) left: Vec<u32>,
    pub(crate) synced: Vec<u32>,
    pub(crate) dmabuf: Vec<String>,
    pub(crate) tables: Vec<(OwnedFd, u32)>,
    pub(crate) created: Vec<wl_buffer::WlBuffer>,
    pub(crate) color: Vec<String>,
}

fn on_wl_callback(
    received: &mut Received,
    proxy: wl_callback::WlCallback,
    event: wl_callback::Event,
) {
    let kind = &proxy.key::<Callback>().expect("the object is tagged");
    if let wl_callback::Event::Done { callback_data } = event {
        received.callbacks.push((*kind, callback_data));
    }
}

fn on_wl_buffer(received: &mut Received, proxy: wl_buffer::WlBuffer, event: wl_buffer::Event) {
    if let wl_buffer::Event::Release = event {
        received.released.push(proxy.id());
    }
}

fn on_wl_surface(received: &mut Received, _: wl_surface::WlSurface, event: wl_surface::Event) {
    match event {
        wl_surface::Event::PreferredBufferScale { factor } => received.buffer_scales.push(factor),
        wl_surface::Event::Enter { output } => received.entered.push(output.id()),
        wl_surface::Event::Leave { output } => received.left.push(output.id()),
        wl_surface::Event::PreferredBufferTransform {
            transform: Ok(transform),
        } => received.buffer_transforms.push(transform.into()),
        _ => {}
    }
}

fn on_wl_shm(received: &mut Received, _: wl_shm::WlShm, event: wl_shm::Event) {
    if let wl_shm::Event::Format { format } = event {
        received.formats.push(raw_value(format));
    }
}

fn on_wp_presentation(
    received: &mut Received,
    _: wp_presentation::WpPresentation,
    event: wp_presentation::Event,
) {
    if let wp_presentation::Event::ClockId { clk_id } = event {
        received.clock = Some(clk_id);
    }
}

fn on_wp_presentation_feedback(
    received: &mut Received,
    proxy: wp_presentation_feedback::WpPresentationFeedback,
    event: wp_presentation_feedback::Event,
) {
    let tag = &proxy.key::<u32>().expect("the object is tagged");
    match event {
        wp_presentation_feedback::Event::Presented {
            tv_sec_hi,
            tv_sec_lo,
            tv_nsec,
            refresh,
            seq_hi,
            seq_lo,
            flags,
        } => received.presented.push(Presented {
            tag: *tag,
            seconds: (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo),
            nanoseconds: tv_nsec,
            refresh,
            sequence: (u64::from(seq_hi) << 32) | u64::from(seq_lo),
            flags: flags.bits(),
        }),
        wp_presentation_feedback::Event::Discarded => received.discarded.push(*tag),
        wp_presentation_feedback::Event::SyncOutput { output } => {
            received.synced.push(output.id());
        }
        _ => {}
    }
}

fn on_wp_fractional_scale_v1(
    received: &mut Received,
    _: wp_fractional_scale_v1::WpFractionalScaleV1,
    event: wp_fractional_scale_v1::Event,
) {
    if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
        received.fractional.push(scale);
    }
}

pub(crate) struct Wayland(Client<Received>);

impl Deref for Wayland {
    type Target = Client<Received>;

    fn deref(&self) -> &Client<Received> {
        &self.0
    }
}

impl DerefMut for Wayland {
    fn deref_mut(&mut self) -> &mut Client<Received> {
        &mut self.0
    }
}

fn listen(queue: &mut Queue<Received>) {
    queue.on::<wl_callback::WlCallback>(on_wl_callback);
    queue.on::<wl_buffer::WlBuffer>(on_wl_buffer);
    queue.on::<wl_surface::WlSurface>(on_wl_surface);
    queue.on::<wl_shm::WlShm>(on_wl_shm);
    queue.on::<wp_presentation::WpPresentation>(on_wp_presentation);
    queue.on::<wp_presentation_feedback::WpPresentationFeedback>(on_wp_presentation_feedback);
    queue.on::<wp_fractional_scale_v1::WpFractionalScaleV1>(on_wp_fractional_scale_v1);
    extensions::listen(queue);
}

impl Wayland {
    pub(crate) fn new(stream: UnixStream) -> Self {
        Self(Client::connect(stream, listen))
    }

    pub(crate) fn compositor(&self) -> wl_compositor::WlCompositor {
        self.global(6)
    }

    pub(crate) fn surface(
        &self,
        compositor: &wl_compositor::WlCompositor,
    ) -> wl_surface::WlSurface {
        compositor.create_surface()
    }
}
