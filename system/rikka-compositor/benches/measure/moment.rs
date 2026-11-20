use std::time::{Duration, Instant};

use super::Plan;
use super::desk::{BTN_LEFT, Desk};
use super::gpu_window::Answered;

const ADE_LIKE: Duration = Duration::from_millis(30);

const DRAG_STEPS: u32 = 12;

const DRAG_STEP: Duration = Duration::from_millis(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moment {
    Maximize,
    MaximizeSlowly,
    Restore,
    Snap,
    Open,
    Close,
    AppMaximize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Cycle {
    pub(crate) began: Instant,
    pub(crate) ended: Instant,
    pub(crate) answered: Option<Answered>,
}

impl Moment {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Moment::Maximize => "own-maximize",
            Moment::MaximizeSlowly => "own-slow-maximize",
            Moment::Restore => "own-restore",
            Moment::Snap => "own-snap",
            Moment::Open => "own-open",
            Moment::Close => "own-close",
            Moment::AppMaximize => "app-maximize",
        }
    }

    pub(crate) fn needs_app(self) -> bool {
        self == Moment::AppMaximize
    }
}

pub(crate) fn run(desk: &mut Desk, moment: Moment, plan: &Plan) -> Vec<Cycle> {
    (0..plan.cycles).map(|_| once(desk, moment, plan)).collect()
}

fn once(desk: &mut Desk, moment: Moment, plan: &Plan) -> Cycle {
    match moment {
        Moment::Maximize | Moment::MaximizeSlowly => {
            let work_time = match moment {
                Moment::MaximizeSlowly => ADE_LIKE,
                _ => Duration::ZERO,
            };
            pause(desk, plan);
            let cycle = observed(desk, plan, |desk| {
                let (window, client) = desk.drawn_window();
                Some(window.maximize(client, true, work_time))
            });
            let (window, client) = desk.drawn_window();
            window.maximize(client, false, Duration::ZERO);
            client.idle_for(plan.settle);
            cycle
        }
        Moment::Restore => {
            let (window, client) = desk.drawn_window();
            window.maximize(client, true, Duration::ZERO);
            client.idle_for(plan.settle);
            pause(desk, plan);
            observed(desk, plan, |desk| {
                let (window, client) = desk.drawn_window();
                Some(window.maximize(client, false, Duration::ZERO))
            })
        }
        Moment::Snap => {
            desk.drawn_moved();
            let at = desk.drawn_at();
            let (width, height) = desk.drawn_window().0.size();
            let middle = (
                at.0 + f64::from(width) / 2.0,
                at.1 + f64::from(height) / 2.0,
            );
            let edge = (1.0, f64::from(desk.extent.height) / 2.0);
            pause(desk, plan);
            drag(desk, middle, edge);
            let cycle = observed(desk, plan, |desk| {
                desk.release(BTN_LEFT);
                let (window, client) = desk.drawn_window();
                Some(window.answer(client, Duration::ZERO))
            });
            let snapped = (
                f64::from(desk.extent.width) / 4.0,
                f64::from(desk.extent.height) / 2.0,
            );
            drag(desk, snapped, middle);
            desk.release(BTN_LEFT);
            let (window, client) = desk.drawn_window();
            window.answer(client, Duration::ZERO);
            client.idle_for(plan.settle);
            desk.drawn_moved();
            cycle
        }
        Moment::Open => {
            desk.close_drawn();
            desk.client.idle_for(plan.rest);
            let mut cycle = observed(desk, plan, |desk| {
                desk.drawn();
                None
            });
            if let Some(shown) = desk.drawn_window().0.first_shown() {
                cycle.began = shown;
            }
            cycle
        }
        Moment::Close => {
            desk.drawn();
            pause(desk, plan);
            let cycle = observed(desk, plan, |desk| {
                desk.close_drawn();
                None
            });
            desk.drawn();
            desk.client.idle_for(plan.settle);
            cycle
        }
        Moment::AppMaximize => {
            pause(desk, plan);
            let cycle = observed(desk, plan, |desk| {
                desk.toggle_maximized();
                None
            });
            desk.toggle_maximized();
            desk.client.idle_for(plan.settle);
            cycle
        }
    }
}

fn pause(desk: &mut Desk, plan: &Plan) {
    if !plan.warm || desk.drawn.is_none() {
        desk.client.idle_for(plan.rest);
        return;
    }
    let until = Instant::now() + plan.rest;
    let mut frame = 0;
    while Instant::now() < until {
        let (window, client) = desk.drawn_window();
        window.flood(client, frame);
        frame += 1;
    }
}

fn observed(
    desk: &mut Desk,
    plan: &Plan,
    act: impl FnOnce(&mut Desk) -> Option<Answered>,
) -> Cycle {
    let began = Instant::now();
    let answered = act(desk);
    desk.client.idle_for(plan.settle);
    Cycle {
        began,
        ended: Instant::now(),
        answered,
    }
}

fn drag(desk: &mut Desk, from: (f64, f64), to: (f64, f64)) {
    desk.grab(from, BTN_LEFT);
    for step in 1..=DRAG_STEPS {
        let along = f64::from(step) / f64::from(DRAG_STEPS);
        desk.point((
            from.0 + (to.0 - from.0) * along,
            from.1 + (to.1 - from.1) * along,
        ));
        std::thread::sleep(DRAG_STEP);
    }
}
