use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, Size};
use rikka_ui::*;
use rikka_widgets::*;

use crate::lists::marking::{Seen, marker};

const VIEW: Size = Size::new(600.0, 500.0);
const FRAME: Duration = Duration::from_millis(16);
const SETTLE_FRAMES: usize = 60;
const SETTLED: usize = 40;
const TAIL: usize = usize::MAX;
const NOTCH: f32 = 48.0;
const GROWTH: f32 = 6.0;
const REACH: f32 = 800.0;

fn height_of(id: usize) -> f32 {
    [40.0, 260.0, 90.0, 700.0, 30.0, 150.0][id % 6]
}

#[derive(State)]
struct Talk {
    tail: f32,
    seen: Rc<Seen>,
}

fn talk_view(talk: &Talk) -> Element {
    let seen = Rc::clone(&talk.seen);
    let tail = talk.tail;
    lazy_column(SETTLED + 1, move |slot| {
        let (id, height) = match slot.index < SETTLED {
            true => (slot.index, height_of(slot.index)),
            false => (TAIL, tail),
        };
        let marker = marker(id, &seen, height);
        container(marker)
            .padding(Inset::edges(20.0, 0.0, 0.0, 0.0))
            .fill_width()
    })
    .padding(Space::LOOSE)
    .estimate(Length::px(120.0))
    .follow(true)
    .fill()
    .build()
}

struct Talking {
    ui: TypedUi<Talk>,
    scene: Scene,
    now: Instant,
    seen: Rc<Seen>,
    bottom: f32,
}

impl Talking {
    fn new() -> Talking {
        let seen = Rc::new(Seen::default());
        let mut ui = TypedUi::new(
            Talk {
                tail: 30.0,
                seen: Rc::clone(&seen),
            },
            talk_view,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        let mut talking = Talking {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
            seen,
            bottom: 0.0,
        };
        talking.rests();
        talking.bottom = talking.foot().expect("tail shown");
        for _ in 0..20 {
            talking.streams();
        }
        talking
    }

    fn frame(&mut self) {
        self.seen.painted.borrow_mut().clear();
        self.now += FRAME;
        self.ui.request_frame();
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.now));
    }

    fn rests(&mut self) {
        for _ in 0..SETTLE_FRAMES {
            self.frame();
        }
    }

    fn grows(&mut self, by: f32) {
        self.ui.update(|talk| talk.tail += by);
        self.frame();
    }

    fn streams(&mut self) {
        self.grows(GROWTH);
    }

    fn scrolls(&mut self, down: f32, source: ScrollSource) {
        self.ui.handle_event(Event::Scroll {
            source,
            position: Point::new(300.0, 200.0),
            delta: ScrollDelta::from_offset(Point::new(0.0, down)),
            modifiers: Modifiers::NONE,
        });
    }

    fn turns(&mut self, down: f32) {
        self.scrolls(down, ScrollSource::Wheel);
        self.rests();
    }

    fn swipes(&mut self, down: f32, steps: usize) {
        self.scrolls(0.0, ScrollSource::Continuous(ScrollPhase::Began));
        for _ in 0..steps {
            self.scrolls(down, ScrollSource::Continuous(ScrollPhase::Moved));
            self.streams();
        }
        self.scrolls(0.0, ScrollSource::Continuous(ScrollPhase::Ended));
        self.rests();
    }

    fn foot(&self) -> Option<f32> {
        let tail = self.ui.state().tail;
        self.seen.at(TAIL).map(|at| at + tail)
    }

    fn follows(&self) -> bool {
        self.foot()
            .is_some_and(|foot| (foot - self.bottom).abs() < 1.0)
    }

    fn reading(&self) -> (usize, f32) {
        self.seen
            .painted
            .borrow()
            .iter()
            .map(|(id, at)| (*id, *at))
            .find(|(id, at)| *id != TAIL && *at > -REACH && *at < VIEW.height)
            .expect("no row shown")
    }

    fn at(&self, id: usize) -> Option<f32> {
        self.seen.at(id)
    }

    fn stays_put(&mut self) -> Result<(), String> {
        let (id, at) = self.reading();
        for _ in 0..40 {
            self.streams();
        }
        match (self.at(id), self.follows()) {
            (Some(now), false) if now == at => Ok(()),
            (now, follows) => Err(format!(
                "row {id} went from {at} to {now:?}, following the foot: {follows}"
            )),
        }
    }

    fn is_carried(&mut self) -> Result<(), String> {
        for _ in 0..40 {
            self.streams();
        }
        match self.follows() {
            true => Ok(()),
            false => Err(format!(
                "bottom at {:?}, expected {}",
                self.foot(),
                self.bottom
            )),
        }
    }
}

type Moves = fn(&mut Talking);

#[test]
fn scrolling_from_bottom_stops_following() {
    let leavings: [(&str, Moves); 2] = [
        ("a slow touchpad swipe", |talking| talking.swipes(-0.4, 60)),
        ("leaving before the foot shrinks under them", |talking| {
            talking.turns(-100.0);
            talking.grows(-200.0);
            talking.rests();
        }),
    ];
    let failed: Vec<String> = leavings
        .into_iter()
        .filter_map(|(name, leaves)| {
            let mut talking = Talking::new();
            leaves(&mut talking);
            talking
                .stays_put()
                .err()
                .map(|why| format!("{name}: {why}"))
        })
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn shrunk_bottom_resumes_following() {
    let mut talking = Talking::new();
    talking.turns(-100.0);
    talking.grows(-200.0);
    talking.rests();
    talking.turns(NOTCH);

    if let Err(why) = talking.is_carried() {
        panic!("{why}");
    }
}
