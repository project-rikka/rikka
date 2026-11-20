use std::time::{Duration, Instant};

use super::Plan;
use super::client::Painted;
use super::desk::{BTN_LEFT, BTN_RIGHT, Desk, paint_window};
use super::moment::{self, Cycle, Moment};
use super::overview::Overview;

const SLOW_CLIENT: Duration = Duration::from_millis(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scenario {
    Idle,
    Spinner,
    Corners,
    Scroll,
    BehindGlass,
    Move,
    Resize,
    OwnRedraw,
    OwnResize,
    OwnResizeSlowly,
    Overview,
    Once(Moment),
}

impl Scenario {
    pub(crate) const ALL: [Scenario; 18] = [
        Scenario::Idle,
        Scenario::Spinner,
        Scenario::Corners,
        Scenario::Scroll,
        Scenario::BehindGlass,
        Scenario::Move,
        Scenario::Resize,
        Scenario::OwnRedraw,
        Scenario::OwnResize,
        Scenario::OwnResizeSlowly,
        Scenario::Overview,
        Scenario::Once(Moment::Maximize),
        Scenario::Once(Moment::MaximizeSlowly),
        Scenario::Once(Moment::Restore),
        Scenario::Once(Moment::Snap),
        Scenario::Once(Moment::Open),
        Scenario::Once(Moment::Close),
        Scenario::Once(Moment::AppMaximize),
    ];

    pub(crate) fn needs_app(self) -> bool {
        matches!(self, Scenario::Once(moment) if moment.needs_app())
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Scenario::Idle => "idle",
            Scenario::Spinner => "spinner",
            Scenario::Corners => "corners",
            Scenario::Scroll => "scroll",
            Scenario::BehindGlass => "behind-glass",
            Scenario::Move => "move",
            Scenario::Resize => "resize",
            Scenario::OwnRedraw => "own-redraw",
            Scenario::OwnResize => "own-resize",
            Scenario::OwnResizeSlowly => "own-slow-resize",
            Scenario::Overview => "overview",
            Scenario::Once(moment) => moment.name(),
        }
    }

    pub(crate) fn prepare(self, desk: &mut Desk) {
        if self.needs_app() {
            desk.launch_app();
        }
        if self.draws() {
            desk.drawn();
        }
        if self == Scenario::Overview {
            let overview = Overview::open(desk);
            desk.overview = Some(overview);
        }
    }

    pub(crate) fn finish(self, desk: &mut Desk) {
        if let Some(overview) = desk.overview.take() {
            overview.close(desk);
        }
    }

    fn draws(self) -> bool {
        match self {
            Scenario::OwnRedraw | Scenario::OwnResize | Scenario::OwnResizeSlowly => true,
            Scenario::Once(moment) => !moment.needs_app(),
            _ => false,
        }
    }
}

pub(crate) fn run(desk: &mut Desk, scenario: Scenario, plan: &Plan) -> Vec<Cycle> {
    let (frames, idle) = (plan.frames, plan.rest);
    match scenario {
        Scenario::Once(moment) => return moment::run(desk, moment, plan),
        Scenario::Idle => {
            let until = Instant::now() + idle;
            while Instant::now() < until {
                std::thread::sleep(Duration::from_millis(50));
                desk.client.roundtrip();
            }
        }
        Scenario::Spinner => {
            for frame in 0..frames {
                let shade = (frame * 37 % 256) as u8;
                desk.front
                    .canvas
                    .paint(60..108, 60..108, |_, _| [shade, 255 - shade, 128, 255]);
                show(desk, Shown::Front, &[(60, 60, 48, 48)]);
            }
        }
        Scenario::Corners => {
            for frame in 0..frames {
                let shade = (frame * 37 % 256) as u8;
                let canvas = &desk.front.canvas;
                let (right, bottom) = (canvas.width - 108, canvas.height - 108);
                let painted = |_, _| [shade, 128, 255 - shade, 255];
                canvas.paint(60..108, 60..108, painted);
                canvas.paint(bottom..bottom + 48, right..right + 48, painted);
                show(
                    desk,
                    Shown::Front,
                    &[(60, 60, 48, 48), (right, bottom, 48, 48)],
                );
            }
        }
        Scenario::Scroll => {
            for frame in 0..frames {
                paint_window(&desk.front, frame as i32 * 6);
                let canvas = &desk.front.canvas;
                let whole = (0, 0, canvas.width, canvas.height);
                show(desk, Shown::Front, &[whole]);
            }
        }
        Scenario::BehindGlass => {
            for frame in 0..frames {
                paint_window(&desk.back, frame as i32 * 6);
                let canvas = &desk.back.canvas;
                let whole = (0, 0, canvas.width, canvas.height);
                show(desk, Shown::Back, &[whole]);
            }
        }
        Scenario::Move => {
            let canvas = &desk.front.canvas;
            let start = (
                desk.front_at.0 + f64::from(canvas.width) / 2.0,
                desk.front_at.1 + f64::from(canvas.height) / 2.0,
            );
            let reach = f64::from(desk.extent.height) / 8.0;
            desk.grab(start, BTN_LEFT);
            for frame in 0..frames {
                let turn = frame as f64 / 90.0 * std::f64::consts::TAU;
                desk.point((
                    start.0 + reach * turn.sin(),
                    start.1 + reach * (1.0 - turn.cos()),
                ));
                tick(desk);
            }
            desk.point(start);
            tick(desk);
            desk.release(BTN_LEFT);
        }
        Scenario::Resize => {
            let canvas = &desk.front.canvas;
            let start = (
                desk.front_at.0 + f64::from(canvas.width) * 0.75,
                desk.front_at.1 + f64::from(canvas.height) * 0.75,
            );
            let reach = f64::from(desk.extent.height) / 10.0;
            desk.grab(start, BTN_RIGHT);
            for frame in 0..frames {
                let turn = frame as f64 / 90.0 * std::f64::consts::TAU;
                let swing = reach * turn.sin();
                desk.point((start.0 + swing, start.1 + swing / 2.0));
                follow_size(desk);
            }
            desk.point(start);
            follow_size(desk);
            desk.release(BTN_RIGHT);
            follow_size(desk);
        }
        Scenario::OwnRedraw => {
            for frame in 0..frames {
                let (window, client) = desk.drawn_window();
                window.redraw(client, frame);
            }
        }
        Scenario::Overview => {
            let mut overview = desk.overview.take().expect("overview opened in prepare");
            overview.swing(desk, frames);
            desk.overview = Some(overview);
        }
        Scenario::OwnResize => resize_drawn(desk, frames, Duration::ZERO),
        Scenario::OwnResizeSlowly => resize_drawn(desk, frames, SLOW_CLIENT),
    }
    Vec::new()
}

fn resize_drawn(desk: &mut Desk, frames: usize, work_time: Duration) {
    let (x, y) = desk.drawn_at();
    let (width, height) = desk.drawn_window().0.size();
    let start = (x + f64::from(width) * 0.75, y + f64::from(height) * 0.75);
    let reach = f64::from(desk.extent.height) / 10.0;
    desk.grab(start, BTN_RIGHT);
    for frame in 0..frames {
        let turn = frame as f64 / 90.0 * std::f64::consts::TAU;
        let swing = reach * turn.sin();
        desk.point((start.0 + swing, start.1 + swing / 2.0));
        let (window, client) = desk.drawn_window();
        window.follow(client, work_time);
    }
    desk.point(start);
    desk.release(BTN_RIGHT);
    let (window, client) = desk.drawn_window();
    window.follow(client, work_time);
    window.follow(client, work_time);
    window.settle(client);
}

#[derive(Clone, Copy)]
enum Shown {
    Front,
    Back,
}

fn show(desk: &mut Desk, shown: Shown, damage: &[(i32, i32, i32, i32)]) {
    let (painted, tag): (&Painted, u32) = match shown {
        Shown::Front => (&desk.front, desk.front_tag()),
        Shown::Back => (&desk.back, desk.back_tag()),
    };
    let before = desk.client.frames(tag);
    desk.client.show(painted, damage);
    desk.client.await_frame(tag, before);
}

fn tick(desk: &mut Desk) {
    let tag = desk.front_tag();
    let before = desk.client.frames(tag);
    desk.client.tick(&desk.front.surface, tag);
    desk.client.await_frame(tag, before);
}

fn follow_size(desk: &mut Desk) {
    let tag = desk.front_tag();
    let configured_size = desk.client.size(tag).filter(|&(width, height)| {
        width > 0
            && height > 0
            && (width, height) != (desk.front.canvas.width, desk.front.canvas.height)
    });
    match configured_size {
        Some((width, height)) => {
            desk.client.repaint(&mut desk.front, width, height);
            paint_window(&desk.front, 0);
            show(desk, Shown::Front, &[(0, 0, width, height)]);
        }
        None => tick(desk),
    }
}
