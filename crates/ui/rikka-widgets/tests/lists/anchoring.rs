use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, Size};
use rikka_ui::*;
use rikka_widgets::*;

use crate::lists::marking::{Seen, marker};

const VIEW: Size = Size::new(500.0, 400.0);
const LINE: f32 = 60.0;
const SETTLE_FRAMES: usize = 8;
const NEWEST: usize = 1000;
const OLDER: usize = usize::MAX;
const OLDER_HEIGHT: f32 = 48.0;

#[derive(State)]
struct History {
    first: usize,
    seen: Rc<Seen>,
}

fn id_at(first: usize, index: usize) -> usize {
    match index {
        0 => OLDER,
        index => first + index - 1,
    }
}

fn height_of(id: usize) -> f32 {
    match id {
        OLDER => OLDER_HEIGHT,
        id => 40.0 + (id % 4) as f32 * 25.0,
    }
}

fn history_view(history: &History) -> Element {
    let seen = Rc::clone(&history.seen);
    let first = history.first;
    lazy_column(1 + NEWEST - first, move |slot| {
        let id = id_at(first, slot.index);
        marker(id, &seen, height_of(id)).keyed(id)
    })
    .named(move |index| id_at(first, index) as u64)
    .estimate(Length::px(120.0))
    .follow(true)
    .fill()
    .build()
}

struct Reading<S: State> {
    ui: TypedUi<S>,
    scene: Scene,
    now: Instant,
    seen: Rc<Seen>,
}

impl<S: State> Reading<S> {
    fn new(state: S, view: fn(&S) -> Element, seen: Rc<Seen>) -> Reading<S> {
        let mut ui = TypedUi::new(state, view, Theme::dark());
        ui.set_viewport(VIEW, 1.0);
        let mut reading = Reading {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
            seen,
        };
        reading.rests();
        reading
    }

    fn rests(&mut self) {
        for _ in 0..SETTLE_FRAMES {
            self.seen.painted.borrow_mut().clear();
            self.now += Duration::from_millis(200);
            self.ui.request_frame();
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.now));
        }
    }

    fn scrolls(&mut self, down: f32) {
        self.ui.handle_event(Event::Scroll {
            source: ScrollSource::Wheel,
            position: Point::new(250.0, 200.0),
            delta: ScrollDelta::from_offset(Point::new(0.0, down)),
            modifiers: Modifiers::NONE,
        });
        self.rests();
    }

    fn at(&self, index: usize) -> Option<f32> {
        self.seen.at(index)
    }

    fn shown(&self) -> Vec<(usize, f32)> {
        self.seen
            .painted
            .borrow()
            .iter()
            .filter(|(_, at)| **at > -LINE && **at < VIEW.height)
            .map(|(index, at)| (*index, *at))
            .collect()
    }
}

#[test]
fn prepend_keeps_scroll_anchor() {
    let seen = Rc::new(Seen::default());
    let mut reading = Reading::new(
        History {
            first: NEWEST - 60,
            seen: Rc::clone(&seen),
        },
        history_view,
        seen,
    );
    reading.scrolls(-1_000_000.0);
    assert_eq!(reading.at(OLDER), Some(0.0), "view not at top");
    let before: Vec<(usize, Option<f32>)> = reading
        .shown()
        .into_iter()
        .filter(|(id, _)| *id != OLDER)
        .map(|(id, at)| (id, Some(at)))
        .collect();

    reading.ui.update(|history| history.first -= 50);
    reading.rests();

    let after: Vec<(usize, Option<f32>)> = before
        .iter()
        .map(|(id, _)| (*id, reading.at(*id)))
        .collect();
    assert_eq!(after, before, "visible content moved");
}
