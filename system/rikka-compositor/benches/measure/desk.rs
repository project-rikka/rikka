use std::time::{Duration, Instant};

use rikka_compositor::{Extent, Injected};
use rikka_wayland_client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use super::app::App;
use super::client::{Client, Painted};
use super::gpu_window::GpuWindow;
use super::overview::Overview;
use super::session::Session;

pub(crate) const BTN_LEFT: u32 = 0x110;
pub(crate) const BTN_RIGHT: u32 = 0x111;
pub(crate) const KEY_LEFTMETA: u32 = 125;
const KEY_M: u32 = 50;

const WALLPAPER: u32 = 1;
const BAR: u32 = 2;
const BACK: u32 = 10;
const FRONT: u32 = 11;
const CLIENT_DRAWN: u32 = 13;
const LAYER_NAMESPACE: &str = "bench";
const PROBES: [(f64, f64); 5] = [(0.4, 0.4), (0.6, 0.6), (0.5, 0.3), (0.3, 0.6), (0.7, 0.4)];

pub(crate) struct Desk {
    pub(crate) drawn: Option<(GpuWindow, Option<(f64, f64)>)>,
    pub(crate) overview: Option<Overview>,
    pub(crate) client: Client,
    pub(crate) extent: Extent,
    pub(crate) back: Painted,
    pub(crate) front: Painted,
    pub(crate) front_at: (f64, f64),
    _wallpaper: Painted,
    _bar: Painted,
    app: Option<App>,
    pub(crate) session: Session,
}

impl Desk {
    pub(crate) fn set(extent: Extent, frosted: bool, millihertz: u32) -> Self {
        let session = Session::start(extent, frosted, millihertz);
        let mut client = Client::connect(session.socket());
        let wallpaper = client.layer(
            WALLPAPER,
            zwlr_layer_shell_v1::Layer::Background,
            zwlr_layer_surface_v1::Anchor::all(),
            (0, 0),
            LAYER_NAMESPACE,
        );
        wallpaper.canvas.paint(
            0..wallpaper.canvas.height,
            0..wallpaper.canvas.width,
            scenery,
        );
        let full = (0, 0, wallpaper.canvas.width, wallpaper.canvas.height);
        client.show(&wallpaper, &[full]);

        let bar_height = (extent.height / 30).max(8);
        let bar = client.layer(
            BAR,
            zwlr_layer_shell_v1::Layer::Top,
            zwlr_layer_surface_v1::Anchor::BOTTOM
                | zwlr_layer_surface_v1::Anchor::LEFT
                | zwlr_layer_surface_v1::Anchor::RIGHT,
            (0, bar_height),
            LAYER_NAMESPACE,
        );
        bar.canvas
            .paint(0..bar.canvas.height, 0..bar.canvas.width, |_, _| glass(0));
        client.frost(&bar.surface, &[(0, 0, bar.canvas.width, bar.canvas.height)]);
        client.show(&bar, &[(0, 0, bar.canvas.width, bar.canvas.height)]);

        let size = (
            (extent.width as i32 * 46 / 100).max(16),
            (extent.height as i32 * 54 / 100).max(16),
        );
        let back = client.window(BACK, size);
        paint_window(&back, 0);
        client.frost(&back.surface, &[(0, 0, size.0, size.1)]);
        client.show(&back, &[(0, 0, size.0, size.1)]);
        let front = client.window(FRONT, size);
        paint_window(&front, 0);
        client.frost(&front.surface, &[(0, 0, size.0, size.1)]);
        client.show(&front, &[(0, 0, size.0, size.1)]);
        client.await_frame(FRONT, 0);

        let mut desk = Self {
            session,
            client,
            extent,
            back,
            front,
            front_at: (0.0, 0.0),
            drawn: None,
            overview: None,
            _wallpaper: wallpaper,
            _bar: bar,
            app: None,
        };
        desk.front_at = desk.locate(FRONT);
        desk
    }

    pub(crate) fn drawn(&mut self) {
        if self.drawn.is_some() {
            return;
        }
        let size = (self.front.canvas.width, self.front.canvas.height);
        let window = GpuWindow::open(&mut self.client, CLIENT_DRAWN, size);
        self.drawn = Some((window, None));
    }

    pub(crate) fn close_drawn(&mut self) {
        if let Some((window, _)) = self.drawn.take() {
            window.close(&mut self.client);
        }
    }

    pub(crate) fn drawn_moved(&mut self) {
        if let Some((_, at)) = &mut self.drawn {
            *at = None;
        }
    }

    pub(crate) fn drawn_at(&mut self) -> (f64, f64) {
        let (window, at) = self.drawn.as_ref().expect("probe window opened");
        if let Some(at) = at {
            return *at;
        }
        let tag = window.tag;
        let at = self.locate(tag);
        if let Some((_, located)) = &mut self.drawn {
            *located = Some(at);
        }
        at
    }

    pub(crate) fn drawn_window(&mut self) -> (&mut GpuWindow, &mut Client) {
        let (window, _) = self.drawn.as_mut().expect("probe window opened");
        (window, &mut self.client)
    }

    fn locate(&mut self, tag: u32) -> (f64, f64) {
        let deadline = Instant::now() + Duration::from_secs(5);
        for (across, down) in PROBES.iter().cycle() {
            let probe = (
                f64::from(self.extent.width) * across,
                f64::from(self.extent.height) * down,
            );
            self.point((probe.0 + 1.0, probe.1 + 1.0));
            self.point(probe);
            match self.client.pointed() {
                Some((seen, x, y)) if seen == tag => return (probe.0 - x, probe.1 - y),
                seen if Instant::now() >= deadline => {
                    panic!("window {tag} is not under any probed point: {seen:?}")
                }
                _ => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        unreachable!("probes run until the deadline")
    }

    pub(crate) fn point(&self, at: (f64, f64)) {
        self.session
            .inject(&[Injected::PointerTo { x: at.0, y: at.1 }]);
    }

    pub(crate) fn launch_app(&mut self) {
        self.close_drawn();
        if self.app.is_none() {
            let app = App::launch(self.session.socket(), self.session.config());
            eprintln!("app log: {}", app.log().display());
            self.app = Some(app);
        }
    }

    pub(crate) fn toggle_maximized(&self) {
        let key = |code, pressed| Injected::Key { code, pressed };
        self.session.inject(&[
            key(KEY_LEFTMETA, true),
            key(KEY_M, true),
            key(KEY_M, false),
            key(KEY_LEFTMETA, false),
        ]);
    }

    pub(crate) fn grab(&self, at: (f64, f64), button: u32) {
        self.session.inject(&[
            Injected::Key {
                code: KEY_LEFTMETA,
                pressed: true,
            },
            Injected::PointerTo { x: at.0, y: at.1 },
            Injected::Button {
                code: button,
                pressed: true,
            },
        ]);
    }

    pub(crate) fn release(&self, button: u32) {
        self.session.inject(&[
            Injected::Button {
                code: button,
                pressed: false,
            },
            Injected::Key {
                code: KEY_LEFTMETA,
                pressed: false,
            },
        ]);
    }

    pub(crate) fn front_tag(&self) -> u32 {
        FRONT
    }

    pub(crate) fn back_tag(&self) -> u32 {
        BACK
    }
}

pub(crate) fn paint_window(window: &Painted, offset: i32) {
    let canvas = &window.canvas;
    canvas.paint(0..canvas.height, 0..canvas.width, |x, y| {
        let line = (y + offset).rem_euclid(28);
        let ink =
            line < 12 && x > 32 && x < canvas.width - 32 && (x / 7 + (y + offset) / 28) % 9 != 0;
        match ink {
            true => [40, 36, 44, 255],
            false => glass(y),
        }
    });
}

fn glass(y: i32) -> [u8; 4] {
    let alpha = 150u32;
    let shade = 230 - (y % 40) as u32;
    let premultiplied = |value: u32| (value * alpha / 255) as u8;
    [
        premultiplied(shade),
        premultiplied(shade - 8),
        premultiplied(shade - 4),
        alpha as u8,
    ]
}

fn scenery(x: i32, y: i32) -> [u8; 4] {
    let stripes = ((x / 23 + y / 17) % 5) as u8 * 18;
    [
        (x / 12 % 256) as u8 ^ stripes,
        (y / 7 % 256) as u8,
        ((x + y) / 19 % 256) as u8 ^ stripes,
        255,
    ]
}
