use std::os::fd::AsFd;
use std::path::Path;
use std::time::Duration;

use rikka_compositor_kms::{
    Buffers, Connector, Device, Dmabuf, DmabufPlane, Gpu, KmsEvent, Output, OutputConfig, Present,
    Session, Target,
};
use rikka_compositor_reactor::Reactor;
use rikka_compositor_types::{Format, FormatSet, Fourcc, Modifier};
use rikka_renderer::{DamageSource, DmabufFence, DmabufTarget, Renderer, Scene};
use rikka_types::Color;

const PATIENCE: Duration = Duration::from_secs(10);
const PAINTED: [u8; 3] = [0xe0, 0x80, 0x20];

#[derive(Default)]
struct Received {
    events: Vec<KmsEvent>,
    timed_out: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Flipped,
    Rejected,
}

struct Display {
    reactor: Reactor<Received>,
    received: Received,
    device: Device,
    connector: Connector,
    _session: Session,
}

impl Display {
    fn open() -> Self {
        unsafe { std::env::set_var("LIBSEAT_BACKEND", "noop") };
        let reactor = Reactor::new().unwrap();
        let session = Session::open(&reactor, |_: &mut Received, _| {}).unwrap();
        let gpu = Gpu::all(session.seat())
            .unwrap()
            .into_iter()
            .find(is_vkms)
            .expect("a vkms display device is present; load it with `sudo modprobe vkms`");
        let events = reactor
            .mailbox(|received: &mut Received, event| received.events.push(event))
            .unwrap();
        let device = Device::open(&session, &gpu, events).unwrap();
        let connector = device
            .connectors()
            .unwrap()
            .into_iter()
            .next()
            .expect("vkms connects a virtual display");
        Self {
            reactor,
            received: Received::default(),
            device,
            connector,
            _session: session,
        }
    }

    fn lighting(&self) -> OutputConfig {
        OutputConfig::new(self.connector.modes()[0])
    }

    fn light(&mut self, buffers: Buffers) -> rikka_compositor_kms::Result<Output> {
        let lighting = self.lighting();
        self.device
            .enable_output(&self.connector, lighting, buffers)
    }

    fn outcome(&mut self, frame: u64) -> Outcome {
        let ends = |received: &Received| {
            received.timed_out
                || received.events.iter().any(|event| match event {
                    KmsEvent::Flipped { presented, .. } => presented.frame == frame,
                    KmsEvent::Rejected { frame: refused, .. } => *refused == frame,
                    KmsEvent::Retired(retired) => retired.frame == frame && !retired.shown,
                    _ => false,
                })
        };
        let patience = self.reactor.after(PATIENCE, |received: &mut Received| {
            received.timed_out = true
        });
        self.reactor.run(&mut self.received, ends).unwrap();
        self.reactor.remove(patience);
        assert!(
            !self.received.timed_out,
            "frame {frame} was neither shown nor refused"
        );
        let shown = self.received.events.iter().any(|event| {
            matches!(event, KmsEvent::Flipped { presented, .. } if presented.frame == frame)
        });
        match shown {
            true => Outcome::Flipped,
            false => Outcome::Rejected,
        }
    }
}

fn is_vkms(gpu: &Gpu) -> bool {
    std::fs::canonicalize(Path::new("/sys/class/drm").join(gpu.name()).join("device"))
        .is_ok_and(|device| device.file_name().is_some_and(|name| name == "vkms"))
}

fn renderer() -> Renderer {
    let gpu = Gpu::all("seat0")
        .unwrap()
        .into_iter()
        .find(|gpu| !is_vkms(gpu))
        .expect("a GPU that renders sits beside vkms");
    Renderer::new(
        &rikka_gpu::Gpu::builder()
            .app_name("rikka-compositor-kms-tests")
            .drm_node(gpu.major(), gpu.minor())
            .build()
            .unwrap(),
    )
    .unwrap()
}

fn linear(formats: &[rikka_gpu::DrmFormat]) -> FormatSet {
    formats
        .iter()
        .map(|format| Format::new(Fourcc(format.fourcc), Modifier(format.modifier)))
        .filter(|format| format.modifier == Modifier::LINEAR)
        .collect()
}

fn paint(renderer: &mut Renderer, target: &mut DmabufTarget) -> DmabufFence {
    let mut scene = Scene::new();
    scene.set_background(Color::rgb8(PAINTED[0], PAINTED[1], PAINTED[2]));
    let mut output = renderer.create_dmabuf_output(DamageSource::CompleteForBuffer);
    renderer
        .draw(&mut scene, output.with_target(target))
        .unwrap()
        .fence
        .expect("a drawn frame hands out a fence")
}

fn present(display: &Display, output: &Output, slot: Target, fence: DmabufFence) -> u64 {
    let present = Present::new(output.crtc(), 1, slot.framebuffer);
    let present = match fence {
        DmabufFence::SyncFile(fence) => present.fence(fence),
        DmabufFence::Readable(readable) => present.ready_when_readable(readable),
    };
    display.device.present(present).unwrap();
    1
}

fn bgr_at(bytes: &[u8], stride: usize, x: usize, y: usize) -> [u8; 3] {
    let at = y * stride + x * 4;
    [bytes[at + 2], bytes[at + 1], bytes[at]]
}

fn near(pixel: [u8; 3], expected: [u8; 3]) -> bool {
    pixel
        .iter()
        .zip(expected)
        .all(|(pixel, expected)| pixel.abs_diff(expected) <= 2)
}

#[test]
fn renderless_device_shows_cpu_writes() {
    let mut display = Display::open();
    let mut output = display
        .light(Buffers::Mapped(vec![Fourcc::XRGB8888]))
        .unwrap();
    let slot = output.swapchain_mut().acquire().unwrap();
    let framebuffer = slot.framebuffer.id();
    let mapped = output.swapchain_mut().mapped(framebuffer).unwrap();
    let stride = mapped.stride() as usize;
    let (width, height) = (mapped.width() as usize, mapped.height() as usize);
    for row in mapped.bytes_mut().chunks_exact_mut(stride).take(height) {
        for pixel in row[..width * 4].as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[PAINTED[2], PAINTED[1], PAINTED[0], 0xff]);
        }
    }
    let present = Present::new(output.crtc(), 1, slot.framebuffer);
    display.device.present(present).unwrap();

    assert_eq!(display.outcome(1), Outcome::Flipped);
    let mapped = output.swapchain_mut().mapped(framebuffer).unwrap();
    assert_eq!(
        bgr_at(mapped.bytes(), stride, width / 2, height / 2),
        PAINTED
    );
}

#[test]
fn primary_gpu_renders_into_secondary_buffers() {
    let mut display = Display::open();
    let mut renderer = renderer();
    let linear = linear(renderer.gpu().dmabuf_render_formats());
    let mut output = match display.light(Buffers::Shared(linear)) {
        Ok(output) => output,
        Err(error) => {
            return eprintln!("vkms allocates no linear buffer for this renderer: {error}");
        }
    };
    let slot = output.swapchain_mut().acquire().unwrap();
    let framebuffer = slot.framebuffer.id();
    let dmabuf = output.swapchain().dmabuf(framebuffer).unwrap();
    assert_eq!(dmabuf.format().modifier, Modifier::LINEAR);
    let mut target = match import(&mut renderer, dmabuf) {
        Ok(target) => target,
        Err(error) => return eprintln!("the renderer cannot draw into vkms memory: {error}"),
    };
    let fence = paint(&mut renderer, &mut target);
    let frame = present(&display, &output, slot, fence);

    assert_eq!(display.outcome(frame), Outcome::Flipped);
    let dmabuf = output.swapchain().dmabuf(framebuffer).unwrap();
    let plane = &dmabuf.planes()[0];
    let length = plane.stride as usize * dmabuf.height() as usize;
    let mapped = unsafe {
        rustix::mm::mmap(
            std::ptr::null_mut(),
            length,
            rustix::mm::ProtFlags::READ,
            rustix::mm::MapFlags::SHARED,
            plane.fd.as_fd(),
            u64::from(plane.offset),
        )
    }
    .unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), length) };
    let pixel = bgr_at(
        bytes,
        plane.stride as usize,
        dmabuf.width() as usize / 2,
        dmabuf.height() as usize / 2,
    );
    unsafe { rustix::mm::munmap(mapped, length) }.unwrap();
    assert!(
        near(pixel, PAINTED),
        "vkms memory holds {pixel:02x?}, not the painted {PAINTED:02x?}"
    );
}

#[test]
fn secondary_gpu_imports_exported_buffers() {
    let mut display = Display::open();
    let mut renderer = renderer();
    let mode = display.lighting().mode();
    let mut targets = Vec::new();
    let mut dmabufs = Vec::new();
    for _ in 0..3 {
        let exported = match renderer.export_shared_dmabuf(
            mode.width(),
            mode.height(),
            Fourcc::XRGB8888.0,
            &[Modifier::LINEAR.0],
        ) {
            Ok(exported) => exported,
            Err(error) => return eprintln!("the renderer exports no linear buffer: {error}"),
        };
        let planes = exported
            .planes
            .iter()
            .map(|plane| DmabufPlane {
                fd: plane.fd.try_clone().unwrap(),
                offset: plane.offset,
                stride: plane.stride,
            })
            .collect();
        dmabufs.push(Dmabuf::new(
            mode.width(),
            mode.height(),
            Format::new(Fourcc::XRGB8888, Modifier::LINEAR),
            planes,
        ));
        targets.push(exported.target);
    }
    let mut output = match display.light(Buffers::Imported(dmabufs)) {
        Ok(output) => output,
        Err(error) => return eprintln!("vkms cannot show what the renderer exported: {error}"),
    };
    let slot = output.swapchain_mut().acquire().unwrap();
    let index = output
        .swapchain()
        .framebuffers()
        .position(|framebuffer| framebuffer.id() == slot.framebuffer.id())
        .unwrap();
    let fence = paint(&mut renderer, &mut targets[index]);
    let frame = present(&display, &output, slot, fence);

    assert_eq!(display.outcome(frame), Outcome::Flipped);
}

#[test]
fn secondary_gpu_shows_cpu_copied_rows() {
    let mut display = Display::open();
    let mut renderer = renderer();
    let mode = display.lighting().mode();
    let (width, height) = (mode.width(), mode.height());
    let mut rows = Vec::new();
    for _ in 0..3 {
        match renderer.export_rows(width, height) {
            Ok(shared) => rows.push(shared),
            Err(error) => return eprintln!("the renderer shares no rows: {error}"),
        }
    }
    let dmabufs = rows
        .iter()
        .map(|shared| {
            Dmabuf::new(
                width,
                height,
                Format::new(Fourcc::XBGR8888, Modifier::LINEAR),
                vec![DmabufPlane {
                    fd: shared.fd.try_clone().unwrap(),
                    offset: 0,
                    stride: shared.stride,
                }],
            )
        })
        .collect();
    let mut output = match display.light(Buffers::Imported(dmabufs)) {
        Ok(output) => output,
        Err(error) => return eprintln!("vkms cannot show the shared rows: {error}"),
    };
    let slot = output.swapchain_mut().acquire().unwrap();
    let index = output
        .swapchain()
        .framebuffers()
        .position(|framebuffer| framebuffer.id() == slot.framebuffer.id())
        .unwrap();
    let mut target = renderer.create_target(width, height).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb8(PAINTED[0], PAINTED[1], PAINTED[2]));
    renderer
        .draw(&mut scene, target.copied_into(&rows[index], &[]))
        .unwrap();
    let copied = renderer.submitted_frame();
    renderer.timeline().wait(copied).unwrap();
    display
        .device
        .present(Present::new(output.crtc(), 1, slot.framebuffer))
        .unwrap();

    assert_eq!(display.outcome(1), Outcome::Flipped);
    let shared = &rows[index];
    let length = shared.stride as usize * height as usize;
    let mapped = unsafe {
        rustix::mm::mmap(
            std::ptr::null_mut(),
            length,
            rustix::mm::ProtFlags::READ,
            rustix::mm::MapFlags::SHARED,
            shared.fd.as_fd(),
            0,
        )
    };
    let Ok(mapped) = mapped else {
        return eprintln!("the shared rows cannot be read by the CPU");
    };
    let bytes = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), length) };
    let at = (height as usize / 2) * shared.stride as usize + (width as usize / 2) * 4;
    let pixel = [bytes[at], bytes[at + 1], bytes[at + 2]];
    unsafe { rustix::mm::munmap(mapped, length) }.unwrap();
    assert!(
        near(pixel, PAINTED),
        "the shared rows hold {pixel:02x?}, not the painted {PAINTED:02x?}"
    );
}

fn import(renderer: &mut Renderer, dmabuf: &Dmabuf) -> rikka_renderer::Result<DmabufTarget> {
    let planes: Vec<rikka_gpu::DmabufPlane<'_>> = dmabuf
        .planes()
        .iter()
        .map(|plane| rikka_gpu::DmabufPlane {
            offset: plane.offset,
            stride: plane.stride,
            fd: plane.fd.as_fd(),
        })
        .collect();
    renderer.import_dmabuf(&rikka_renderer::DmabufSpec {
        width: dmabuf.width(),
        height: dmabuf.height(),
        fourcc: dmabuf.format().fourcc.0,
        modifier: dmabuf.format().modifier.0,
        planes: &planes,
    })
}
