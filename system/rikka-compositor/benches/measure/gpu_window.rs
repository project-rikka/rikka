use std::time::{Duration, Instant};

use rikka_present::Presenter;
use rikka_renderer::{
    Acquired, FrameStart, NodeId, Present, Recorded, Renderer, Scene, SurfaceConfig,
};
use rikka_types::{Color, Rect};
use rikka_wayland::Host;
use rikka_wayland_client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use rikka_wayland_client::wl_surface;

use super::client::Client;

const RADIUS: f32 = 16.0;

const LINES: usize = 16;

const PATIENCE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy)]
pub(crate) struct Presented {
    pub(crate) at: Instant,
    pub(crate) drawing: Duration,
    pub(crate) answered: Option<Duration>,
    pub(crate) resized: bool,
    pub(crate) configures: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Answered {
    pub(crate) configured: Instant,
    pub(crate) committed: Instant,
    pub(crate) drawing: Duration,
}

pub(crate) struct GpuWindow {
    pub(crate) tag: u32,
    surface: wl_surface::WlSurface,
    effect: ExtBackgroundEffectSurfaceV1,
    presenter: Presenter,
    scene: Scene,
    renderer: Renderer,
    ground: NodeId,
    lines: Vec<NodeId>,
    size: (i32, i32),
    answered: usize,
    pub(crate) presented: Vec<Presented>,
}

impl GpuWindow {
    pub(crate) fn open(client: &mut Client, tag: u32, size: (i32, i32)) -> Self {
        let surface = client.toplevel(tag);
        let effect = client.frost(&surface, &[(0, 0, size.0, size.1)]);
        let mut renderer = Renderer::new(
            &rikka_gpu::Gpu::builder()
                .app_name("frame-cost-client")
                .build()
                .expect("Vulkan device that exports dmabufs"),
        )
        .expect("Vulkan device that exports dmabufs");
        let config = SurfaceConfig {
            transparent: true,
            ..SurfaceConfig::default()
        };
        let host = Host::read_by_owner(client.display());
        let mut presenter =
            Presenter::new(&host, &surface, &mut renderer, config).expect("window presenter");
        client.roundtrip();
        presenter.resize((size.0 as u32, size.1 as u32), 1.0, false);
        let mut scene = Scene::new();
        scene.set_background(Color::TRANSPARENT);
        let ground = scene.insert(
            rikka_renderer::Node::rounded_rect(whole(size), RADIUS)
                .fill(Color::rgba(0.96, 0.95, 0.98, 0.55)),
        );
        let lines = (0..LINES)
            .map(|line| {
                scene.insert(
                    rikka_renderer::Node::rect(line_rect(size, line))
                        .fill(Color::rgba(0.15, 0.14, 0.18, 0.9)),
                )
            })
            .collect();
        let answered = client.configured(tag).len();
        let mut window = Self {
            tag,
            surface,
            effect,
            renderer,
            presenter,
            scene,
            ground,
            lines,
            size,
            answered,
            presented: Vec::new(),
        };
        window.present(client, false);
        window
    }

    pub(crate) fn close(self, client: &mut Client) {
        let (surface, tag) = (self.surface.clone(), self.tag);
        drop(self);
        client.destroy(tag, &surface);
        client.roundtrip();
    }

    pub(crate) fn first_shown(&self) -> Option<Instant> {
        self.presented
            .first()
            .map(|presented| presented.at - presented.drawing)
    }

    pub(crate) fn size(&self) -> (i32, i32) {
        self.size
    }

    pub(crate) fn redraw(&mut self, client: &mut Client, frame: usize) {
        let shade = (frame % 31 + 1) as f32 / 32.0;
        let line = self.lines[frame % self.lines.len()];
        self.scene
            .edit(line)
            .fill(Color::rgba(0.15 + 0.5 * shade, 0.14, 0.18, 0.9));
        self.present(client, false);
    }

    pub(crate) fn flood(&mut self, client: &mut Client, frame: usize) {
        let shade = (frame % 31 + 1) as f32 / 32.0;
        self.scene
            .edit(self.ground)
            .fill(Color::rgba(0.96, 0.95 - 0.1 * shade, 0.98, 0.55));
        self.present(client, false);
    }

    pub(crate) fn follow(&mut self, client: &mut Client, work_time: Duration) {
        client.roundtrip();
        let configured_size = client
            .size(self.tag)
            .filter(|&(width, height)| width > 0 && height > 0 && (width, height) != self.size);
        match configured_size {
            Some(size) => {
                std::thread::sleep(work_time);
                self.size = size;
                self.fit(true);
                self.reshape(client, size);
                self.present(client, true);
            }
            None => {
                let before = client.frames(self.tag);
                client.tick(&self.surface, self.tag);
                client.await_frame(self.tag, before);
            }
        }
    }

    pub(crate) fn maximize(
        &mut self,
        client: &mut Client,
        maximized: bool,
        work_time: Duration,
    ) -> Answered {
        client.maximize(self.tag, maximized);
        self.answer(client, work_time)
    }

    pub(crate) fn answer(&mut self, client: &mut Client, work_time: Duration) -> Answered {
        let size = client.await_resize(self.tag, self.size);
        let configured = *client
            .configured(self.tag)
            .last()
            .expect("configure carries a new size");
        std::thread::sleep(work_time);
        self.size = size;
        self.fit(false);
        self.reshape(client, size);
        self.present(client, true);
        let presented = *self.presented.last().expect("new size presented");
        Answered {
            configured,
            committed: presented.at,
            drawing: presented.drawing,
        }
    }

    fn reshape(&mut self, client: &mut Client, size: (i32, i32)) {
        client.blur(&self.effect, &[(0, 0, size.0, size.1)]);
        self.scene.edit(self.ground).rect(whole(size));
        for (line, node) in self.lines.iter().enumerate() {
            self.scene.edit(*node).rect(line_rect(size, line));
        }
    }

    pub(crate) fn settle(&mut self, client: &mut Client) {
        self.fit(false);
        self.present(client, false);
    }

    fn fit(&mut self, interactive: bool) {
        let (width, height) = (self.size.0 as u32, self.size.1 as u32);
        self.presenter.resize((width, height), 1.0, interactive);
    }

    fn present(&mut self, client: &mut Client, resized: bool) {
        let configured = client.configured(self.tag);
        let configures = configured.len() - self.answered;
        let configured_at = configured.last().copied().filter(|_| resized);
        self.answered = configured.len();
        let before = client.frames(self.tag);
        let started = Instant::now();
        let recorded = record(&mut self.renderer, &mut self.scene, &mut self.presenter);
        client.ask_frame(&self.surface, self.tag);
        let output = self.presenter.present(recorded);
        match output {
            Ok(output) => assert!(output.rendered(), "client frame drew nothing"),
            Err(error) => panic!("cannot present client frame: {error}"),
        }
        let presented = Instant::now();
        self.presented.push(Presented {
            at: presented,
            drawing: presented - started,
            answered: configured_at.map(|configured_at| presented - configured_at),
            resized,
            configures,
        });
        client.await_frame(self.tag, before);
    }
}

fn record(renderer: &mut Renderer, scene: &mut Scene, presenter: &mut Presenter) -> Recorded {
    let started = presenter
        .start(renderer, scene)
        .expect("client frame begins");
    assert_eq!(started, FrameStart::Draw, "client scene updated");
    match presenter
        .acquire_image(PATIENCE)
        .expect("client acquires a buffer")
    {
        Acquired::Image(image) => presenter
            .record_image(renderer, scene, image)
            .expect("client records its frame"),
        Acquired::Stale | Acquired::NotReady => panic!("compositor holds every buffer"),
    }
}

fn whole(size: (i32, i32)) -> Rect {
    Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32)
}

fn line_rect(size: (i32, i32), line: usize) -> Rect {
    let width = (size.0 as f32 - 64.0).max(8.0);
    let top = 48.0 + line as f32 * 28.0;
    Rect::new(
        32.0,
        top,
        width * (0.5 + 0.5 * ((line * 7 % 11) as f32 / 11.0)),
        12.0,
    )
}
