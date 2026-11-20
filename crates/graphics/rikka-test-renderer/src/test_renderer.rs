use rikka_sync::{Mutex, MutexGuard};
use std::ops::{Deref, DerefMut};

use rikka_gpu::{Gpu, GpuBuilder};
use rikka_renderer::{Renderer, Result, Uploads};

static GPU_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug)]
pub struct TestRenderer {
    renderer: Renderer,
    gpu_lock: GpuLock,
}

#[derive(Debug)]
pub struct GpuLock {
    _guard: MutexGuard<'static, ()>,
}

impl TestRenderer {
    pub fn split(self) -> (GpuLock, Renderer) {
        (self.gpu_lock, self.renderer)
    }
}

impl Deref for TestRenderer {
    type Target = Renderer;

    fn deref(&self) -> &Renderer {
        &self.renderer
    }
}

impl DerefMut for TestRenderer {
    fn deref_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }
}

impl AsRef<Uploads> for TestRenderer {
    fn as_ref(&self) -> &Uploads {
        self.renderer.as_ref()
    }
}

pub fn lock_gpu() -> GpuLock {
    GpuLock {
        _guard: GPU_LOCK.lock(),
    }
}

pub fn renderer(app_name: &str) -> Result<TestRenderer> {
    build_renderer(app_name, |builder| builder)
}

pub fn video_renderer(app_name: &str) -> Result<TestRenderer> {
    build_renderer(app_name, |builder| builder.video_decode().video_encode())
}

pub fn streaming_renderer(app_name: &str) -> Result<TestRenderer> {
    build_renderer(app_name, GpuBuilder::upload_queue)
}
pub fn compute_renderer(app_name: &str) -> Result<TestRenderer> {
    build_renderer(app_name, GpuBuilder::compute_support)
}

pub fn renderer_3d(app_name: &str) -> Result<TestRenderer> {
    build_renderer(app_name, GpuBuilder::with_3d)
}

pub fn fluid_renderer(app_name: &str) -> Result<TestRenderer> {
    build_renderer(app_name, |builder| builder.with_3d().compute_support())
}

pub fn ray_renderer(app_name: &str) -> Result<TestRenderer> {
    build_renderer(app_name, GpuBuilder::ray_tracing)
}

fn build_renderer(
    app_name: &str,
    configure: impl FnOnce(GpuBuilder) -> GpuBuilder,
) -> Result<TestRenderer> {
    crate::init_tracing();
    let gpu_lock = lock_gpu();
    let renderer = Renderer::new(&configure(Gpu::builder().app_name(app_name)).build()?)?;
    Ok(TestRenderer { renderer, gpu_lock })
}
