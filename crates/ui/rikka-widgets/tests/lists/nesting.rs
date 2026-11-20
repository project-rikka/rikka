use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

use crate::lists::marking::{Seen, marker};

const VIEW: Size = Size::new(600.0, 500.0);
const POINTER: Point = Point::new(300.0, 250.0);
const FRAME: Duration = Duration::from_millis(16);
const CELLS: usize = 30;
const CELL: f32 = 150.0;
const NESTED: usize = 3;
const HEADING: f32 = 10.0;
const ROWS: usize = 40;
const ROW: f32 = 28.0;
const FIRST_ROW: usize = 1000;
const TRAVEL: f32 = 400.0;
const CODE_LINES: usize = 4;
const CODE_HEIGHT: f32 = 160.0;
const CODE_TRAVEL: f32 = 280.0;

#[derive(Clone, State)]
struct Page {
    seen: Rc<Seen>,
    code: TextDocument,
    nests_code: bool,
}

fn inner(page: &Page) -> Element {
    if page.nests_code {
        return text_editor(&page.code)
            .read_only(true)
            .wrap(false)
            .gutter(false)
            .height(Length::px(CODE_HEIGHT))
            .fill_width()
            .build();
    }
    let seen = Rc::clone(&page.seen);
    lazy_column(ROWS, move |slot| marker(FIRST_ROW + slot.index, &seen, ROW))
        .estimate(Length::px(ROW))
        .max_height(200.0)
        .fill_width()
        .build()
}

fn page_view(page: &Page) -> Element {
    let page = page.clone();
    lazy_column(CELLS, move |slot| match slot.index == NESTED {
        true => column! {
            marker(NESTED, &page.seen, HEADING);
            inner(&page);
        }
        .fill_width()
        .build(),
        false => marker(slot.index, &page.seen, CELL),
    })
    .estimate(Length::px(CELL))
    .fill()
    .build()
}

struct Reading {
    ui: TypedUi<Page>,
    scene: Scene,
    now: Instant,
    seen: Rc<Seen>,
}

impl Reading {
    fn new(nests_code: bool) -> Reading {
        let seen = Rc::new(Seen::default());
        let wide = "wide ".repeat(200);
        let mut ui = TypedUi::new(
            Page {
                seen: Rc::clone(&seen),
                code: TextDocument::new(vec![wide; CODE_LINES].join("\n")),
                nests_code,
            },
            page_view,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        let mut reading = Reading {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
            seen,
        };
        reading.frames(30);
        reading
    }

    fn frames(&mut self, count: usize) {
        for _ in 0..count {
            self.seen.painted.borrow_mut().clear();
            self.now += FRAME;
            self.ui.request_frame();
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.now));
        }
    }

    fn scrolls(&mut self, offset: Point, source: ScrollSource) {
        self.ui.handle_event(Event::Scroll {
            source,
            position: POINTER,
            delta: ScrollDelta::from_offset(offset),
            modifiers: Modifiers::NONE,
        });
        self.frames(1);
    }

    fn at(&self, id: usize) -> f32 {
        self.seen.at(id).unwrap_or_else(|| panic!("{id} not shown"))
    }

    fn page_top(&mut self) -> f32 {
        self.frames(30);
        self.at(NESTED)
    }

    fn nested_offset(&mut self) -> f32 {
        let top = self.page_top() + HEADING;
        top - self.at(FIRST_ROW)
    }
}

type Gesture = fn(&mut Reading, f32);

fn notches(reading: &mut Reading, travel: f32) {
    let notch = 40.0;
    for _ in 0..(travel / notch) as usize {
        reading.scrolls(Point::new(0.0, notch), ScrollSource::Wheel);
    }
}

fn swipe(reading: &mut Reading, travel: f32) {
    let step = 8.0;
    reading.scrolls(Point::ZERO, ScrollSource::Continuous(ScrollPhase::Began));
    for _ in 0..(travel / step) as usize {
        reading.scrolls(
            Point::new(0.0, step),
            ScrollSource::Continuous(ScrollPhase::Moved),
        );
    }
    reading.scrolls(Point::ZERO, ScrollSource::Continuous(ScrollPhase::Ended));
}

#[test]
fn gesture_stays_with_origin_list() {
    let gestures: [(&str, Gesture); 2] = [("wheel notches", notches), ("a touchpad swipe", swipe)];
    let failed: Vec<String> = gestures
        .into_iter()
        .filter_map(|(name, gesture)| {
            let mut reading = Reading::new(false);
            let from = reading.page_top();
            gesture(&mut reading, TRAVEL);
            let travelled = from - reading.page_top();
            let nested = reading.nested_offset();
            (travelled != TRAVEL || nested != 0.0).then(|| {
                format!("{name}: the page went {travelled}, the list under the pointer {nested}")
            })
        })
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");

    let mut reading = Reading::new(false);
    notches(&mut reading, TRAVEL);
    let page = reading.page_top();
    swipe(&mut reading, 80.0);
    assert_eq!(
        (reading.page_top(), reading.nested_offset()),
        (page, 80.0),
        "nested list gesture scrolled outer list"
    );
}

#[test]
fn vertical_swipe_over_code_scrolls_page() {
    let mut reading = Reading::new(true);
    notches(&mut reading, CODE_TRAVEL);
    let from = reading.page_top();
    reading.scrolls(Point::ZERO, ScrollSource::Continuous(ScrollPhase::Began));
    for _ in 0..10 {
        reading.scrolls(
            Point::new(1.0, 8.0),
            ScrollSource::Continuous(ScrollPhase::Moved),
        );
    }
    assert_eq!(from - reading.page_top(), 80.0, "code block took the swipe");
}
