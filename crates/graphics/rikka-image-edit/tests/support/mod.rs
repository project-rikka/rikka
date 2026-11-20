use rikka_sync::{Mutex, MutexGuard};

use rikka_gpu::ComputedFrame;
use rikka_image::ColorType;
use rikka_image_edit::{Imaging, Picture, Recipe};
use rikka_renderer::{Node, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_types::Rect;

static GPU_LOCK: Mutex<()> = Mutex::new(());

struct Engine {
    imaging: Imaging,
    _gpu_lock: MutexGuard<'static, ()>,
}

thread_local! {
    static ENGINE: Engine = {
        rikka_test_renderer::init_tracing();
        let gpu_lock = GPU_LOCK.lock();
        Engine {
            imaging: Imaging::headless().expect("Vulkan 1.3 compute device required"),
            _gpu_lock: gpu_lock,
        }
    };
}

pub(crate) fn imaging() -> Imaging {
    ENGINE.with(|engine| engine.imaging.clone())
}

pub(crate) struct Screen {
    imaging: Imaging,
    renderer: TestRenderer,
    _gpu_lock: MutexGuard<'static, ()>,
}

pub(crate) fn screen() -> Screen {
    let gpu_lock = GPU_LOCK.lock();
    let renderer = rikka_test_renderer::compute_renderer("rikka-image-edit-screen")
        .expect("Vulkan 1.3 compute device required");
    let compute = renderer.gpu().compute().expect("compute queue available");
    Screen {
        imaging: Imaging::new(compute),
        renderer,
        _gpu_lock: gpu_lock,
    }
}

impl Screen {
    pub(crate) fn imaging(&self) -> &Imaging {
        &self.imaging
    }

    pub(crate) fn drawn(&mut self, frame: &ComputedFrame) -> Vec<u8> {
        let (width, height) = (frame.width(), frame.height());
        let texture = self
            .renderer
            .textures()
            .create_compute_target(width, height)
            .expect("texture created");
        for (at, part) in frame.parts() {
            self.renderer
                .uploads()
                .copy_computed(texture, *at, part.clone())
                .expect("part copies");
        }
        let mut target = self
            .renderer
            .create_target(width, height)
            .expect("render target");
        let mut scene = Scene::new();
        scene.insert(Node::image(
            Rect::new(0.0, 0.0, width as f32, height as f32),
            texture,
        ));
        self.renderer
            .draw(&mut scene, &mut target)
            .expect("scene renders");
        self.renderer
            .read_pixels(&mut target)
            .expect("pixels download")
    }
}

pub(crate) fn picture(width: u32, height: u32, texel: impl Fn(u32, u32) -> [u8; 4]) -> Picture {
    let pixels: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| texel(x, y))
        .collect();
    imaging()
        .upload_rgba8(width, height, &pixels)
        .expect("valid picture")
}

pub(crate) fn flat(width: u32, height: u32, texel: [u8; 4]) -> Picture {
    picture(width, height, |_, _| texel)
}

pub(crate) fn rgba(picture: &Picture) -> Vec<u8> {
    picture
        .to_image()
        .expect("picture downloads")
        .convert(ColorType::Rgba8)
        .as_bytes()
        .to_vec()
}

pub(crate) fn at(picture: &Picture, x: u32, y: u32) -> [u8; 4] {
    let pixels = rgba(picture);
    let index = ((y * picture.width() + x) * 4) as usize;
    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

pub(crate) fn developed(picture: &Picture, recipe: Recipe) -> Picture {
    picture.develop(&recipe).expect("recipe develops")
}

pub(crate) fn close(a: u8, b: u8, tolerance: u8) -> bool {
    a.abs_diff(b) <= tolerance
}
