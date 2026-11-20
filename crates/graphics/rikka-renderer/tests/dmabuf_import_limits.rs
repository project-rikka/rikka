use std::fs::OpenOptions;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use gbm::{BufferObject, BufferObjectFlags, Format as GbmFormat, Modifier};
use rikka_gpu::{DRM_FORMAT_MOD_LINEAR, DmabufPlane};
use rikka_renderer::{DmabufSpec, Error, Renderer};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const XRGB8888: u32 = GbmFormat::Xrgb8888 as u32;
const RENDER_NODE: &str = "/dev/dri/renderD128";

struct Setup {
    renderer: Renderer,
    bo: BufferObject<()>,
    fds: Vec<OwnedFd>,
    modifier: u64,
    _gpu_lock: rikka_test_renderer::GpuLock,
}

impl Setup {
    fn new() -> Option<Self> {
        if !Path::new(RENDER_NODE).exists() {
            eprintln!("{RENDER_NODE} is absent; skipping");
            return None;
        }
        let node = OpenOptions::new()
            .read(true)
            .write(true)
            .open(RENDER_NODE)
            .expect("the render node opens");
        let rdev = node.metadata().expect("render node metadata").rdev();

        rikka_test_renderer::init_tracing();
        let gpu_lock = rikka_test_renderer::lock_gpu();
        let renderer = Renderer::new(
            &rikka_gpu::Gpu::builder()
                .app_name("rikka-dmabuf-limits-test")
                .drm_node(libc::major(rdev), libc::minor(rdev))
                .build()
                .expect("a Vulkan device drives the render node"),
        )
        .expect("a Vulkan device drives the render node");
        let sampleable: Vec<u64> = renderer
            .gpu()
            .dmabuf_texture_formats()
            .iter()
            .filter(|format| format.fourcc == XRGB8888)
            .map(|format| format.modifier)
            .collect();
        let modifier = renderer
            .gpu()
            .dmabuf_render_formats()
            .iter()
            .filter(|format| format.fourcc == XRGB8888)
            .map(|format| format.modifier)
            .filter(|modifier| sampleable.contains(modifier))
            .min_by_key(|modifier| *modifier != DRM_FORMAT_MOD_LINEAR)?;

        let gbm = gbm::Device::new(node).expect("a GBM device on the render node");
        let bo = gbm
            .create_buffer_object_with_modifiers2::<()>(
                WIDTH,
                HEIGHT,
                GbmFormat::Xrgb8888,
                [Modifier::from(modifier)].into_iter(),
                BufferObjectFlags::RENDERING,
            )
            .expect("an XRGB8888 buffer object the GPU can draw and sample");
        let fds = (0..bo.plane_count() as i32)
            .map(|plane| bo.fd_for_plane(plane).expect("the buffer exports a dmabuf"))
            .collect();
        Some(Self {
            renderer,
            bo,
            fds,
            modifier,
            _gpu_lock: gpu_lock,
        })
    }
}

fn planes_of<'a>(bo: &BufferObject<()>, fds: &'a [OwnedFd]) -> Vec<DmabufPlane<'a>> {
    fds.iter()
        .enumerate()
        .map(|(index, fd)| DmabufPlane {
            offset: bo.offset(index as i32),
            stride: bo.stride_for_plane(index as i32),
            fd: fd.as_fd(),
        })
        .collect()
}

fn refuses(renderer: &mut Renderer, spec: &DmabufSpec<'_>) {
    let drawn = renderer.import_dmabuf(spec);
    assert!(
        matches!(drawn, Err(Error::Invalid(_))),
        "a render target was imported from {spec:?}"
    );
    let sampled = renderer.textures().import_dmabuf(spec);
    assert!(
        matches!(sampled, Err(Error::Invalid(_))),
        "a texture was imported from {spec:?}"
    );
}

#[test]
fn rejects_plane_outside_layout() {
    let Some(mut setup) = Setup::new() else {
        return;
    };
    let mut planes = planes_of(&setup.bo, &setup.fds);
    planes.push(DmabufPlane {
        offset: 0,
        stride: planes[0].stride,
        fd: setup.fds[0].as_fd(),
    });
    let spec = DmabufSpec {
        width: WIDTH,
        height: HEIGHT,
        fourcc: XRGB8888,
        modifier: setup.modifier,
        planes: &planes,
    };
    refuses(&mut setup.renderer, &spec);
}

#[test]
fn rejects_import_beyond_device_limits() {
    let Some(mut setup) = Setup::new() else {
        return;
    };
    let planes = planes_of(&setup.bo, &setup.fds);
    let spec = DmabufSpec {
        width: 1 << 20,
        height: 1 << 20,
        fourcc: XRGB8888,
        modifier: setup.modifier,
        planes: &planes,
    };
    refuses(&mut setup.renderer, &spec);
}
