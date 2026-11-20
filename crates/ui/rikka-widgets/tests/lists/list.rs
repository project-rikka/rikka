use std::cell::RefCell;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, Size};
use rikka_ui::*;
use rikka_widgets::*;

use crate::common::wakes;

const VIEW: Size = Size::new(500.0, 400.0);
const ITEMS: usize = 300;
const ITEM: f32 = 200.0;
const SETTLE_FRAMES: usize = 8;

#[derive(Debug, Default)]
struct Built {
    requested: Vec<usize>,
}

impl Built {
    fn window(&self) -> (usize, usize) {
        let first = self.requested.iter().copied().min().unwrap_or(0);
        let last = self.requested.iter().copied().max().unwrap_or(0);
        (first, last)
    }
}

#[derive(State)]
struct Feed {
    follow: bool,
    built: Rc<RefCell<Built>>,
}

impl Feed {
    fn new(follow: bool) -> Feed {
        Feed {
            follow,
            built: Rc::new(RefCell::new(Built::default())),
        }
    }
}

fn feed_view(feed: &Feed) -> Element {
    let built = Rc::clone(&feed.built);
    lazy_column(ITEMS, move |slot| {
        let index = slot.index;
        built.borrow_mut().requested.push(index);
        container(text(format!("item {index}")))
            .height(Length::px(ITEM))
            .fill_width()
    })
    .estimate(Length::px(ITEM))
    .follow(feed.follow)
    .fill()
    .build()
}

fn framed<S: State>(
    ui: &mut TypedUi<S>,
    now: Instant,
    scene: &mut Scene,
    frames: usize,
) -> Instant {
    let mut now = now;
    for _ in 0..frames {
        now += Duration::from_millis(200);
        ui.request_frame();
        ui.render(Rendering::new(scene).with_time(now));
    }
    now
}

fn open_feed(follow: bool) -> (TypedUi<Feed>, Scene, Instant) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Feed::new(follow), feed_view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let now = framed(&mut ui, Instant::now(), &mut scene, SETTLE_FRAMES);
    (ui, scene, now)
}

fn scrolled<S: State>(ui: &mut TypedUi<S>, steps: usize, down: f32) {
    for _ in 0..steps {
        ui.handle_event(Event::Scroll {
            source: ScrollSource::Wheel,
            position: Point::new(250.0, 200.0),
            delta: ScrollDelta::from_offset(Point::new(0.0, down)),
            modifiers: Modifiers::NONE,
        });
    }
}

fn watched(ui: &TypedUi<Feed>) -> Rc<RefCell<Built>> {
    Rc::clone(&ui.state().built)
}

#[test]
fn only_visible_items_are_built() {
    let (ui, _scene, _now) = open_feed(false);
    let (_, last) = watched(&ui).borrow().window();

    assert!(last < 10, "built up to {last}, expected about one screen");
}

#[test]
fn scrolling_builds_revealed_items() {
    let (mut ui, mut scene, now) = open_feed(false);
    let built = watched(&ui);
    built.borrow_mut().requested.clear();

    scrolled(&mut ui, 10, 400.0);
    framed(&mut ui, now, &mut scene, SETTLE_FRAMES);

    let reached = (10.0 * 400.0 / ITEM) as usize;
    let log = built.borrow();
    assert!(
        log.requested.contains(&reached),
        "{:?} never reached item {reached}",
        log.window()
    );
    assert!(!log.requested.contains(&(ITEMS - 1)), "unreached end built");
}

#[test]
fn idle_list_prefetches_next_screen() {
    let (mut ui, mut scene, now) = open_feed(false);
    let built = watched(&ui);
    scrolled(&mut ui, 1, ITEM);
    let mut now = framed(&mut ui, now, &mut scene, SETTLE_FRAMES);
    let (_, reached) = built.borrow().window();

    let mut idle = 0;
    while ui.wants_prefetch() && idle < 32 {
        ui.prefetch(&mut scene, Instant::now() + Duration::from_secs(1));
        now = framed(&mut ui, now, &mut scene, 1);
        idle += 1;
    }
    let (_, ahead) = built.borrow().window();
    let screen = (VIEW.height / ITEM) as usize;
    let far_edge = 1 + screen;
    assert!(
        ahead > reached && ahead >= far_edge + screen,
        "idle prefetch reached {ahead}, expected one screen past {far_edge}"
    );

    built.borrow_mut().requested.clear();
    scrolled(&mut ui, screen, ITEM);
    framed(&mut ui, now, &mut scene, SETTLE_FRAMES);
    let rebuilt: Vec<usize> = built
        .borrow()
        .requested
        .iter()
        .copied()
        .filter(|index| *index <= ahead)
        .collect();
    assert!(rebuilt.is_empty(), "{rebuilt:?} rebuilt on arrival");
}

fn shown(seen: &Rc<Seen>) -> Vec<(usize, Point)> {
    let mut painted = seen.painted.borrow_mut();
    let mut drawn: Vec<(usize, Point)> = painted
        .iter()
        .copied()
        .filter(|(_, at)| at.y > -ITEM && at.y < VIEW.height)
        .collect();
    painted.clear();
    drawn.sort_by_key(|(index, _)| *index);
    drawn.dedup_by_key(|(index, _)| *index);
    drawn
}

#[derive(Debug, Default)]
struct Seen {
    painted: RefCell<Vec<(usize, Point)>>,
}

struct Marker {
    index: usize,
    seen: Rc<Seen>,
    width: rikka_ui::Length,
    height: f32,
}

impl rikka_ui::Widget for Marker {
    type State = ();

    fn layout(&self) -> rikka_ui::Layout {
        rikka_ui::Layout::row().size(self.width, rikka_ui::Length::px(self.height))
    }

    fn paint(&self, (): &mut (), ctx: &mut rikka_ui::PaintCtx<'_>) {
        self.seen
            .painted
            .borrow_mut()
            .push((self.index, ctx.bounds().origin));
    }
}

#[derive(State)]
struct Uneven {
    seen: Rc<Seen>,
}

fn uneven_view(uneven: &Uneven) -> Element {
    let seen = Rc::clone(&uneven.seen);
    lazy_column(ITEMS, move |slot| {
        Element::new(Marker {
            index: slot.index,
            seen: Rc::clone(&seen),
            width: rikka_ui::Length::FILL,
            height: match slot.index % 3 {
                0 => 40.0,
                _ => 360.0,
            },
        })
    })
    .estimate(Length::px(ITEM))
    .fill()
    .build()
}

#[test]
fn mixed_heights_keep_scroll_anchor() {
    let mut scene = Scene::new();
    let seen = Rc::new(Seen::default());
    let mut ui = TypedUi::new(
        Uneven {
            seen: Rc::clone(&seen),
        },
        uneven_view,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);

    let mut now = Instant::now();
    let mut drawn = |ui: &mut TypedUi<Uneven>, scene: &mut Scene| {
        now = framed(ui, now, scene, SETTLE_FRAMES);
        shown(&seen)
    };

    drawn(&mut ui, &mut scene);
    for _ in 0..12 {
        ui.handle_event(Event::Scroll {
            source: ScrollSource::Wheel,
            position: Point::new(250.0, 200.0),
            delta: ScrollDelta::from_offset(Point::new(0.0, 400.0)),
            modifiers: Modifiers::NONE,
        });
    }

    drawn(&mut ui, &mut scene);
    let reached = drawn(&mut ui, &mut scene);
    assert!(!reached.is_empty(), "nothing drawn after scrolling");

    for _ in 0..4 {
        assert_eq!(
            drawn(&mut ui, &mut scene),
            reached,
            "scroll anchor drifted during estimation"
        );
    }
}

const GUESS: f32 = 120.0;
const SHORT: f32 = 70.0;
const LONG: f32 = 520.0;

fn tall(index: usize, contents: u64) -> f32 {
    match (index as u64 + contents) % 4 {
        0 => LONG,
        _ => SHORT,
    }
}

#[derive(State)]
struct Talk {
    contents: u64,
    count: usize,
    looking: Option<usize>,
    away: bool,
    place: Place,
    seen: Rc<Seen>,
}

fn talk_view(talk: &Talk) -> Element {
    if talk.away {
        return container(spacer()).fill().build();
    }
    let seen = Rc::clone(&talk.seen);
    let contents = talk.contents;
    lazy_column(talk.count, move |slot| {
        Element::new(Marker {
            index: slot.index,
            seen: Rc::clone(&seen),
            width: rikka_ui::Length::FILL,
            height: tall(slot.index, contents),
        })
    })
    .estimate(Length::px(GUESS))
    .follow(true)
    .shows(contents)
    .focus(talk.looking)
    .place(&talk.place)
    .fill()
    .build()
}

struct Talking {
    ui: TypedUi<Talk>,
    scene: Scene,
    now: Instant,
    seen: Rc<Seen>,
}

impl Talking {
    fn new(count: usize) -> Talking {
        let seen = Rc::new(Seen::default());
        let mut ui = TypedUi::new(
            Talk {
                contents: 1,
                count,
                looking: None,
                away: false,
                place: Place::new(),
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
        };
        talking.rests();
        talking
    }

    fn rests(&mut self) {
        self.now = framed(&mut self.ui, self.now, &mut self.scene, SETTLE_FRAMES);
        self.draws();
    }

    fn draws(&mut self) {
        self.seen.painted.borrow_mut().clear();
        self.now = framed(&mut self.ui, self.now, &mut self.scene, 1);
    }

    fn arrives(&mut self, blocks: usize) {
        self.ui.update(|talk| talk.count += blocks);
        self.rests();
    }

    fn scrolls(&mut self, steps: usize, down: f32) {
        scrolled(&mut self.ui, steps, down);
        self.rests();
    }

    fn foot(&self) -> Option<f32> {
        let talk = self.ui.state();
        let last = talk.count - 1;
        self.seen
            .painted
            .borrow()
            .iter()
            .find(|(index, _)| *index == last)
            .map(|(index, at)| at.y + tall(*index, talk.contents))
    }

    fn ended(&self) -> bool {
        self.foot()
            .is_some_and(|foot| (foot - VIEW.height).abs() < 2.0)
    }

    fn painted(&self) -> Vec<(usize, f32)> {
        let ended: BTreeMap<usize, f32> = self
            .seen
            .painted
            .borrow()
            .iter()
            .map(|(index, at)| (*index, at.y))
            .collect();
        ended.into_iter().collect()
    }
}

#[test]
fn bottom_follows_appended_items() {
    let mut talking = Talking::new(200);
    talking.arrives(10);

    assert!(
        talking.ended(),
        "append left bottom view behind: {:?}",
        talking.painted()
    );
}

#[test]
fn scrolled_away_does_not_follow() {
    let mut talking = Talking::new(200);
    talking.scrolls(3, -300.0);
    let reading = talking.painted();

    talking.arrives(5);

    assert!(!talking.ended(), "append did not follow at bottom");
    assert_eq!(talking.painted(), reading, "append below moved view");
}

#[test]
fn exact_bottom_resumes_following() {
    let mut talking = Talking::new(200);
    talking.scrolls(3, -300.0);
    talking.scrolls(1, 890.0);

    assert!(!talking.ended(), "near-bottom view snapped to bottom");
    talking.arrives(5);
    assert!(
        !talking.ended(),
        "append snapped near-bottom view to bottom"
    );

    talking.scrolls(1, 2000.0);
    talking.arrives(5);
    assert!(
        talking.ended(),
        "bottom-anchored view stopped following: {:?}",
        talking.painted()
    );
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

static COVER_GATE: Mutex<Option<Receiver<()>>> = Mutex::new(None);

#[derive(State)]
struct Covers;

fn covers_view(_: &Covers) -> Element {
    lazy_column(1, |_| {
        let cover = fetch_blocking("a-cell-reading-a-fetch", || {
            let gate = COVER_GATE.lock().expect("gate mutex poisoned").take();
            if let Some(gate) = gate {
                gate.recv().expect("test dropped the gate sender");
            }
            Ok::<_, Infallible>("cover landed")
        });
        text(cover.ready().copied().unwrap_or("cover waiting")).any()
    })
    .fill()
    .any()
}

#[test]
fn cell_shows_fetched_value() {
    let (open, gate) = channel();
    *COVER_GATE.lock().expect("gate mutex poisoned") = Some(gate);
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Covers, covers_view, Theme::dark());
    let woken = wakes(&mut ui);
    ui.set_viewport(VIEW, 1.0);
    ui.render(Rendering::new(&mut scene).with_time(Instant::now()));
    assert!(ui.dump().contains("cover waiting"));

    open.send(()).expect("fetch thread stopped");
    while !ui.dump().contains("cover landed") {
        assert!(
            woken.wait(FETCH_TIMEOUT),
            "fetched value never shown:\n{}",
            ui.dump()
        );
        ui.render(Rendering::new(&mut scene).with_time(Instant::now()));
    }
}
