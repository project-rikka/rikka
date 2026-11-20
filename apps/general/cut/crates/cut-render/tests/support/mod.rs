#![allow(
    dead_code,
    reason = "the test and machine binaries each use part of the shared setup"
)]

use rikka_sync::{Mutex, MutexGuard};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::{Duration, Instant};

use cut_project::{
    Area, Art, ClipId, Edit, Graphic, Item, Media, Picture as Shot, Project, Rate, Region,
    Shot as View, Viewpoint,
};
use cut_render::{Composite, Compositor, FrameInputs, Request};
use rikka_effects::{Effect, Kind};
use rikka_figure::{Look, Shape};
use rikka_image::ColorType;
use rikka_image_edit::{Imaging, Picture};
use rikka_types::{Color, Point, Rect};
use rikka_video::{EncodeSpeed, EncoderBackend, EncoderConfig, VideoEncoder};

static GPU_LOCK: Mutex<()> = Mutex::new(());

const TIMEOUT: Duration = Duration::from_secs(30);
const SLACK: f32 = 2.0;

pub(crate) struct Engine {
    pub(crate) imaging: Imaging,
    _gpu_lock: MutexGuard<'static, ()>,
}

pub(crate) fn engine() -> Engine {
    let gpu_lock = GPU_LOCK.lock();
    Engine {
        imaging: Imaging::headless().expect("a Vulkan 1.3 device that can compute is required"),
        _gpu_lock: gpu_lock,
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        rikka_video::wait_for_closed_players();
    }
}

pub(crate) struct Studio {
    pub(crate) compositor: Compositor,
    ready: Receiver<()>,
}

impl Studio {
    pub(crate) fn new(engine: &Engine) -> Studio {
        let mut compositor = Compositor::new(engine.imaging.clone());
        let (wake, ready) = sync_channel(1);
        compositor.on_ready(move || {
            let _ = wake.try_send(());
        });
        Studio { compositor, ready }
    }

    pub(crate) fn ready_inputs(&mut self, request: &Request) -> FrameInputs {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let inputs = self.compositor.collect_inputs(request, request.frame());
            if inputs.is_ready() {
                return inputs;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            self.ready
                .recv_timeout(left)
                .expect("every frame arrives in time");
        }
    }

    pub(crate) fn compose(&mut self, request: &Request) -> Composite {
        let inputs = self.ready_inputs(request);
        self.compositor
            .compose(request, request.frame(), &inputs)
            .expect("composed")
    }
}

pub(crate) fn pixels(picture: &Picture) -> Vec<u8> {
    picture
        .to_image()
        .expect("the picture comes back")
        .convert(ColorType::Rgba8)
        .as_bytes()
        .to_vec()
}

pub(crate) fn pixel_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    [0, 1, 2, 3].map(|channel| pixels[index + channel])
}

pub(crate) fn is_near(actual: [u8; 4], expected: [u8; 4], slack: u8) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| actual.abs_diff(expected) <= slack)
}

pub(crate) fn square(side: f32, fill: Color) -> Graphic {
    Graphic::new(Art::Shape {
        shape: Shape::Rectangle { radius: 0.0 },
        width: side,
        height: side,
        look: Look {
            fill: Some(fill),
            outline: None,
        },
    })
}

pub(crate) fn band(width: f32, height: f32) -> Region {
    Region {
        area: Area::Shape {
            shape: Shape::Rectangle { radius: 0.0 },
            width,
            height,
        },
        ..Region::whole()
    }
}

pub(crate) fn insert(project: &Project, item: impl Into<Item>, layer: usize) -> (Project, ClipId) {
    let (project, applied) = project
        .apply(&Edit::Insert {
            item: item.into(),
            at: 0,
            length: 60,
            layer,
        })
        .expect("inserting");
    (project, applied.selected.expect("the inserted item")[0])
}

pub(crate) fn apply(project: &Project, edit: Edit) -> Project {
    project.apply(&edit).expect("applying").0
}

pub(crate) fn effect(kind: Kind, settings: &[(&str, f32)]) -> Effect {
    let mut effect = kind.default_effect();
    for (key, value) in settings {
        assert!(effect.set(key, *value), "{kind:?} has a knob {key}");
    }
    effect
}

pub(crate) fn request(project: Project, frame: i64, scale: f32) -> Request {
    Request::new(Arc::new(project), frame).scale(scale)
}

pub(crate) fn level(index: u32) -> u8 {
    (16 + index * 5) as u8
}

pub(crate) fn encode_test_video(
    path: &Path,
    width: u32,
    height: u32,
    fps: u32,
    frames: u32,
) -> Option<Media> {
    let config = EncoderConfig::new(width, height)
        .frame_rate(fps)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Balanced)
        .quality(100);
    let Ok(mut encoder) = VideoEncoder::new(None, path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return None;
    };
    for index in 0..frames {
        let grey = level(index);
        encoder
            .push_frame(&[grey, grey, grey, 255].repeat((width * height) as usize))
            .expect("push frame");
    }
    encoder.finish().expect("finish");
    Some(
        Media::new(
            path,
            Duration::from_secs_f64(f64::from(frames) / f64::from(fps)),
        )
        .with_picture(Shot {
            width,
            height,
            rate: Rate::new(fps, 1).expect("a rate"),
        }),
    )
}

pub(crate) fn framed_project() -> Project {
    insert(&Project::new(), Region::whole(), 0).0
}

pub(crate) fn place(project: &Project, media: Media, layer: usize) -> (Project, ClipId) {
    let (project, applied) = project
        .apply(&Edit::Place {
            media: vec![media],
            at: 0,
            layer,
        })
        .expect("placing");
    (project, applied.selected.expect("the placed clip")[0])
}

pub(crate) struct Oracle {
    shot: View,
    local: Rect,
    scale: f32,
}

#[derive(Debug, Default)]
pub(crate) struct Tally {
    pub(crate) inside: usize,
    pub(crate) outside: usize,
    pub(crate) wrong: Vec<(u32, u32)>,
}

impl Oracle {
    pub(crate) fn new(
        project: &Project,
        clip: ClipId,
        frame: i64,
        view: Viewpoint,
        local: Rect,
        scale: f32,
    ) -> Oracle {
        let subject = project.locate(clip).expect("the clip").1;
        Oracle {
            shot: project.shot(subject, frame, view).expect("a shot"),
            local,
            scale,
        }
    }

    pub(crate) fn pixel(&self, local: Point) -> (u32, u32) {
        let at = self.shot.projection.map(local).expect("a seen point") * self.scale;
        (at.x as u32, at.y as u32)
    }

    pub(crate) fn tally(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
        painted: impl Fn([u8; 4]) -> bool,
        bare: impl Fn([u8; 4]) -> bool,
    ) -> Tally {
        let mut tally = Tally::default();
        for y in 0..height {
            for x in 0..width {
                let pixel = pixel_at(rgba, width, x, y);
                match self.expected(x, y) {
                    Some(true) => {
                        tally.inside += 1;
                        if !painted(pixel) {
                            tally.wrong.push((x, y));
                        }
                    }
                    Some(false) => {
                        tally.outside += 1;
                        if !bare(pixel) {
                            tally.wrong.push((x, y));
                        }
                    }
                    None => {}
                }
            }
        }
        tally
    }

    fn expected(&self, x: u32, y: u32) -> Option<bool> {
        let (left, top) = (x as f32 - SLACK, y as f32 - SLACK);
        let (right, bottom) = (x as f32 + 1.0 + SLACK, y as f32 + 1.0 + SLACK);
        let corners = [(left, top), (right, top), (right, bottom), (left, bottom)]
            .map(|(x, y)| self.inside(x, y));
        match corners {
            [true, true, true, true] => Some(true),
            [false, false, false, false] => Some(false),
            _ => None,
        }
    }

    fn inside(&self, x: f32, y: f32) -> bool {
        self.shot
            .projection
            .unmap(Point::new(x / self.scale, y / self.scale))
            .is_some_and(|local| self.local.contains(local))
    }
}
