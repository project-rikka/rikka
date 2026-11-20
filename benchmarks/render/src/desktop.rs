use rikka_gpu::Pixels;
use rikka_renderer::{
    BackdropCache, Blur, Damage, Node, NodeId, OffscreenTarget, Renderer, Result, Scene, TextureId,
};
use rikka_types::{Color, Lens, Rect};

pub(crate) const SCREEN: (u32, u32) = (2560, 1440);

const BLUR_RADIUS: f32 = 48.0;

const CORNER: f32 = 20.0;

const PATCH: (u32, u32) = (40, 40);

const VIDEO: (u32, u32) = (480, 270);

const DRAG_REACH: f32 = 180.0;

const PHASES: usize = 8;

struct Pane {
    key: u64,
    node: NodeId,
}

struct Window {
    node: NodeId,
    texture: TextureId,
    rect: Rect,
}

struct Patches {
    size: (u32, u32),
    phases: Vec<Vec<u8>>,
}

impl Patches {
    fn new(size: (u32, u32)) -> Patches {
        Patches {
            size,
            phases: (0..PHASES).map(|phase| patch(size, phase)).collect(),
        }
    }

    fn at(&self, frame: usize) -> Pixels {
        Pixels::premultiplied(
            self.size.0,
            self.size.1,
            self.phases[frame % PHASES].clone(),
        )
    }
}

pub(crate) struct Desktop {
    pub(crate) scene: Scene,
    panes: Vec<Pane>,
    windows: Vec<Window>,
    menu: usize,
    keys: Vec<u64>,
    spinner: Patches,
    video: Patches,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Spinner,
    VideoUnderMenu,
    Everything { blurs_kept: bool },
    Drag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layout {
    Busy,
    Lone,
}

pub(crate) fn area(damage: &Damage) -> f32 {
    damage.iter().map(|rect| rect.width() * rect.height()).sum()
}

fn wallpaper(width: u32, height: u32) -> Pixels {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            let wave = ((u * 23.0).sin() * (v * 17.0).cos() * 0.5 + 0.5) * 0.35;
            let r = (0.25 + 0.6 * u + wave).min(1.0);
            let g = (0.35 + 0.4 * v).min(1.0);
            let b = (0.85 - 0.5 * u + wave * 0.5).min(1.0);
            data.extend([r, g, b].map(|c| (c * 255.0) as u8));
            data.push(255);
        }
    }
    Pixels::premultiplied(width, height, data)
}

fn window_pixels(width: u32, height: u32, tint: [f32; 3]) -> Pixels {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let bar = y < 48 || (y % 28 < 14 && x > 40 && x < width - 40 && (x / 9) % 7 != 0);
            let (alpha, shade): (f32, f32) = match bar {
                true => (0.92, 0.2),
                false => (0.62, 1.0),
            };
            data.extend(tint.map(|c| (c * shade * alpha * 255.0) as u8));
            data.push((alpha * 255.0) as u8);
        }
    }
    Pixels::premultiplied(width, height, data)
}

fn patch((width, height): (u32, u32), frame: usize) -> Vec<u8> {
    let phase = frame as f32 * 0.7;
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let wave = ((x as f32 * 0.03 + phase).sin() + (y as f32 * 0.04 - phase).cos()) * 0.25;
            data.extend([0.5 + wave, 0.4 - wave, 0.6 + wave * 0.5].map(|c| (c * 255.0) as u8));
            data.push(255);
        }
    }
    data
}

impl Desktop {
    pub(crate) fn new(renderer: &mut Renderer, layout: Layout) -> Result<Desktop> {
        let (width, height) = SCREEN;
        let screen = Rect::new(0.0, 0.0, width as f32, height as f32);
        let mut scene = Scene::new();
        scene.set_background(Color::BLACK);
        let backdrop = renderer.textures().create(wallpaper(width, height))?;
        scene.insert(Node::image(screen, backdrop));

        let busy = [
            (Rect::new(160.0, 120.0, 1475.0, 975.0), [0.97, 0.97, 0.99]),
            (Rect::new(1180.0, 320.0, 1200.0, 900.0), [0.95, 0.96, 0.99]),
            (Rect::new(520.0, 640.0, 900.0, 640.0), [0.99, 0.97, 0.95]),
            (Rect::new(1720.0, 90.0, 700.0, 500.0), [0.96, 0.99, 0.97]),
            (Rect::new(380.0, 300.0, 320.0, 420.0), [0.98, 0.98, 0.98]),
        ];
        let lone = [(Rect::new(690.0, 300.0, 1180.0, 780.0), [0.97, 0.97, 0.99])];
        let placed = match layout {
            Layout::Busy => &busy[..],
            Layout::Lone => &lone[..],
        };
        let lens = Lens::new().scaled(1.25);
        let mut panes = Vec::new();
        let mut windows = Vec::new();
        let mut key = 0;
        let mut glass = |scene: &mut Scene, rect: Rect, z: i32, fill: Color| {
            key += 1;
            let node = scene.insert(
                Node::rounded_rect(rect, CORNER)
                    .fill(fill)
                    .backdrop(Blur::with_radius(BLUR_RADIUS))
                    .backdrop_cache(BackdropCache::current(key))
                    .lens(lens)
                    .z(z),
            );
            panes.push(Pane { key, node });
        };
        glass(
            &mut scene,
            Rect::new(0.0, 0.0, width as f32, 44.0),
            100,
            Color::WHITE.with_alpha(0.3),
        );
        glass(
            &mut scene,
            Rect::new(930.0, 1340.0, 700.0, 84.0),
            100,
            Color::WHITE.with_alpha(0.3),
        );
        for (z, &(rect, tint)) in placed.iter().enumerate() {
            let z = 2 * z as i32 + 1;
            glass(&mut scene, rect, z, Color::TRANSPARENT);
            let texture = renderer.textures().create(window_pixels(
                rect.width() as u32,
                rect.height() as u32,
                tint,
            ))?;
            let node = scene.insert(Node::image(rect, texture).radii(CORNER).z(z + 1));
            windows.push(Window {
                node,
                texture,
                rect,
            });
        }
        let keys = panes.iter().map(|pane| pane.key).collect();
        Ok(Desktop {
            scene,
            menu: windows.len() - 1,
            panes,
            windows,
            keys,
            spinner: Patches::new(PATCH),
            video: Patches::new(VIDEO),
        })
    }

    pub(crate) fn change(
        &mut self,
        renderer: &mut Renderer,
        change: Change,
        frame: usize,
    ) -> Result<()> {
        renderer.retain_backdrops(&self.keys);
        match change {
            Change::Spinner => {
                let window = self.windows[1].rect;
                let at = [window.width() as u32 - 80, 12];
                let pixels = self.spinner.at(frame);
                self.repaint(renderer, 1, at, pixels)?;
            }
            Change::VideoUnderMenu => {
                let menu = self.windows[self.menu].rect;
                let under = self.windows[0].rect;
                let at = [
                    (menu.x() - under.x()) as u32 + 40,
                    (menu.y() - under.y()) as u32 + 60,
                ];
                let pixels = self.video.at(frame);
                let stale = self.repaint(renderer, 0, at, pixels)?;
                let menu_pane = self.panes.len() - 1;
                self.keep(
                    menu_pane,
                    BackdropCache::stale(self.panes[menu_pane].key, [stale]),
                );
            }
            Change::Everything { blurs_kept } => {
                self.scene.damage_all();
                if !blurs_kept {
                    let (width, height) = SCREEN;
                    let screen = Rect::new(0.0, 0.0, width as f32, height as f32);
                    for index in 0..self.panes.len() {
                        self.keep(index, BackdropCache::stale(self.panes[index].key, [screen]));
                    }
                }
            }
            Change::Drag => {
                let turn = frame as f32 / 90.0 * std::f32::consts::TAU;
                let home = self.windows[0].rect;
                let rect = home.translate(rikka_types::Point::new(
                    DRAG_REACH * turn.sin(),
                    DRAG_REACH * (1.0 - turn.cos()),
                ));
                let (width, height) = SCREEN;
                let screen = Rect::new(0.0, 0.0, width as f32, height as f32);
                let pane = self.panes.len() - 1;
                let key = self.panes[pane].key;
                self.scene
                    .edit(self.panes[pane].node)
                    .rect(rect)
                    .backdrop_cache(BackdropCache::stale(key, [screen]));
                self.scene.edit(self.windows[0].node).rect(rect);
            }
        }
        Ok(())
    }

    fn keep(&mut self, index: usize, keep: BackdropCache) {
        self.scene.edit(self.panes[index].node).backdrop_cache(keep);
    }

    fn repaint(
        &mut self,
        renderer: &mut Renderer,
        window: usize,
        at: [u32; 2],
        pixels: Pixels,
    ) -> Result<Rect> {
        let window = &self.windows[window];
        let [width, height] = pixels.extent();
        renderer
            .textures()
            .update_region(window.texture, at, pixels)?;
        let changed = Rect::new(
            window.rect.x() + at[0] as f32,
            window.rect.y() + at[1] as f32,
            width as f32,
            height as f32,
        );
        self.scene.damage_rect(changed);
        Ok(changed)
    }

    pub(crate) fn draw(
        &mut self,
        renderer: &mut Renderer,
        onto: &mut OffscreenTarget,
    ) -> Result<f32> {
        renderer
            .draw(&mut self.scene, onto)
            .map(|drawn| area(&drawn.damage))
    }
}
