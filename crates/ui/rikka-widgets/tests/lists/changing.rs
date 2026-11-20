use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(300.0, 200.0);
const ROW: f32 = 20.0;
const HEADER: f32 = 12.0;
const HEADERS: u64 = 1000;
const FRAME: Duration = Duration::from_millis(16);
const FRAMES: usize = 40;

#[derive(Clone, Copy, Debug)]
struct Painted {
    id: u64,
    painter: u64,
    at: f32,
    from: f32,
    to: f32,
}

#[derive(Default)]
struct Seen {
    painted: RefCell<Vec<Painted>>,
    painters: Cell<u64>,
}

fn extent(id: u64) -> f32 {
    match id >= HEADERS {
        true => HEADER,
        false => ROW,
    }
}

struct Marker {
    id: u64,
    seen: Rc<Seen>,
}

impl Widget for Marker {
    type State = Option<u64>;

    fn behavior(&self, _painter: &Option<u64>) -> Behavior {
        Behavior::PAINTS_OFFSCREEN
    }

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(extent(self.id)))
    }

    fn paint(&self, painter: &mut Option<u64>, ctx: &mut PaintCtx<'_>) {
        let painter = *painter.get_or_insert_with(|| {
            let next = self.seen.painters.get() + 1;
            self.seen.painters.set(next);
            next
        });
        let visible = ctx.visible();
        self.seen.painted.borrow_mut().push(Painted {
            id: self.id,
            painter,
            at: ctx.bounds().y(),
            from: visible.y(),
            to: visible.y() + visible.height(),
        });
    }
}

#[derive(State)]
struct Rows {
    ids: Vec<u64>,
    measured: bool,
    seen: Rc<Seen>,
}

fn rows_view(rows: &Rows) -> Element {
    let seen = Rc::clone(&rows.seen);
    let ids: Rc<[u64]> = rows.ids.clone().into();
    let drawn = Rc::clone(&ids);
    let column = lazy_column(ids.len(), move |slot| {
        let id = drawn[slot.index];
        Element::new(Marker {
            id,
            seen: Rc::clone(&seen),
        })
        .keyed(id)
    })
    .named(move |index| ids[index]);
    let column = match rows.measured {
        true => column.estimate(Length::px(ROW)),
        false => column.uniform(Length::px(ROW)),
    };
    column.fill().build()
}

type Change = (usize, fn(&mut Vec<u64>));

struct Case {
    named: &'static str,
    rows: Vec<u64>,
    measured: bool,
    scrolled: f32,
    changes: Vec<Change>,
    still: Vec<u64>,
}

struct Watch {
    ui: TypedUi<Rows>,
    scene: Scene,
    now: Instant,
    seen: Rc<Seen>,
}

impl Watch {
    fn new(rows: &[u64], measured: bool, scrolled: f32) -> Watch {
        let seen = Rc::new(Seen::default());
        let mut ui = TypedUi::new(
            Rows {
                ids: rows.to_vec(),
                measured,
                seen: Rc::clone(&seen),
            },
            rows_view,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        let mut watch = Watch {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
            seen,
        };
        watch.rests();
        if scrolled != 0.0 {
            watch.ui.handle_event(Event::Scroll {
                source: ScrollSource::Wheel,
                position: Point::new(VIEW.width / 2.0, VIEW.height / 2.0),
                delta: ScrollDelta::from_offset(Point::new(0.0, scrolled)),
                modifiers: Modifiers::NONE,
            });
            watch.rests();
        }
        watch
    }

    fn frame(&mut self, by: Duration) -> Vec<Painted> {
        self.seen.painted.borrow_mut().clear();
        self.now += by;
        self.ui.request_frame();
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.now));
        let mut painters = HashSet::new();
        let mut shown: Vec<Painted> = Vec::new();
        for painted in self.seen.painted.borrow().iter().rev() {
            if !painters.insert(painted.painter) {
                continue;
            }
            let twice = shown
                .iter()
                .any(|seen| seen.id == painted.id && (seen.at - painted.at).abs() < 0.5);
            if painted.to - painted.from > 0.5 && !twice {
                shown.push(*painted);
            }
        }
        shown.sort_by(|a, b| a.at.total_cmp(&b.at));
        shown
    }

    fn rests(&mut self) -> Vec<Painted> {
        (0..8)
            .map(|_| self.frame(Duration::from_millis(200)))
            .last()
            .unwrap_or_default()
    }
}

fn overlapping(shown: &[Painted]) -> Option<(u64, u64)> {
    shown.iter().enumerate().find_map(|(at, a)| {
        shown[at + 1..]
            .iter()
            .find(|b| a.id != b.id && a.to.min(b.to) - a.from.max(b.from) > 0.5)
            .map(|b| (a.id, b.id))
    })
}

fn layout(shown: &[Painted]) -> Vec<(u64, i32)> {
    shown
        .iter()
        .map(|painted| (painted.id, painted.at.round() as i32))
        .collect()
}

fn watched(case: &Case) {
    let mut watch = Watch::new(&case.rows, case.measured, case.scrolled);
    let before = watch.frame(FRAME);
    let mut changes = case.changes.iter().peekable();
    let mut frames = Vec::new();
    for frame in 0..FRAMES {
        while let Some((_, change)) = changes.next_if(|(at, _)| *at == frame) {
            watch.ui.update(|rows| change(&mut rows.ids));
        }
        frames.push(watch.frame(FRAME));
    }
    let settled = watch.rests();

    for (frame, shown) in frames.iter().enumerate() {
        assert_eq!(
            overlapping(shown),
            None,
            "{}: rows overlap at frame {frame}: {:?}",
            case.named,
            layout(shown)
        );
        for still in &case.still {
            let was = before.iter().find(|painted| painted.id == *still);
            let now = shown.iter().find(|painted| painted.id == *still);
            if let (Some(was), Some(now)) = (was, now) {
                assert!(
                    (was.at - now.at).abs() < 0.5,
                    "{}: row {still} moved from {} to {} at frame {frame}",
                    case.named,
                    was.at,
                    now.at
                );
            }
        }
    }

    let moving = case.changes[0].0 + 3;
    assert_ne!(
        layout(&frames[moving]),
        layout(&settled),
        "{}: change applied without motion",
        case.named
    );

    settled_in_order(case.named, &watch.ui.state().ids, &settled);
}

fn settled_in_order(named: &str, order: &[u64], settled: &[Painted]) {
    let shown: Vec<u64> = settled.iter().map(|painted| painted.id).collect();
    let first = order.iter().position(|id| Some(id) == shown.first());
    let expected: Vec<u64> = first
        .map(|first| order[first..first + shown.len()].to_vec())
        .unwrap_or_default();
    assert_eq!(shown, expected, "{named}: rows settled out of order");
    for pair in settled.windows(2) {
        assert!(
            (pair[1].at - pair[0].at - extent(pair[0].id)).abs() < 0.5,
            "{named}: rows settled apart: {:?}",
            layout(settled)
        );
    }
}

fn rows(count: u64) -> Vec<u64> {
    (0..count).collect()
}

fn insert_children(ids: &mut Vec<u64>, at: usize, from: u64, count: u64) {
    for (offset, id) in (from..from + count).enumerate() {
        ids.insert(at + offset, id);
    }
}

#[test]
fn rows_never_overlap_during_changes() {
    let cases = [
        Case {
            named: "a reordered row",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| {
                let moved = ids.remove(6);
                ids.insert(0, moved);
            })],
            still: vec![7, 8, 9],
        },
        Case {
            named: "a row inserted mid-list",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| ids.insert(4, 100))],
            still: vec![0, 1, 2, 3],
        },
        Case {
            named: "a removed row",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| {
                ids.remove(4);
            })],
            still: vec![0, 1, 2, 3],
        },
        Case {
            named: "an opened folder",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| insert_children(ids, 3, 100, 4))],
            still: vec![0, 1, 2],
        },
        Case {
            named: "a closed folder",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| drop(ids.drain(3..7)))],
            still: vec![0, 1, 2],
        },
        Case {
            named: "a folder longer than the view",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| insert_children(ids, 3, 100, 40))],
            still: vec![0, 1, 2],
        },
        Case {
            named: "a folder closed past the foot",
            rows: rows(60),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| drop(ids.drain(3..50)))],
            still: vec![0, 1, 2],
        },
        Case {
            named: "a replaced row",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| ids[4] = 100)],
            still: vec![0, 1, 2, 3, 5, 6, 7, 8],
        },
        Case {
            named: "scattered changes",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![(1, |ids| {
                ids.remove(8);
                ids.insert(5, 300);
                ids.remove(2);
                ids.insert(0, 301);
            })],
            still: vec![3, 4, 9],
        },
        Case {
            named: "a folder closed while it opens",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![
                (1, |ids| insert_children(ids, 3, 100, 4)),
                (6, |ids| drop(ids.drain(3..7))),
            ],
            still: vec![0, 1, 2],
        },
        Case {
            named: "a second folder opened mid-motion",
            rows: rows(12),
            measured: false,
            scrolled: 0.0,
            changes: vec![
                (1, |ids| insert_children(ids, 2, 100, 3)),
                (5, |ids| {
                    let at = ids.iter().position(|id| *id == 4).map_or(0, |at| at + 1);
                    insert_children(ids, at, 200, 3);
                }),
            ],
            still: vec![0, 1],
        },
        Case {
            named: "a header and row in a measured list",
            rows: rows(12),
            measured: true,
            scrolled: 0.0,
            changes: vec![(1, |ids| {
                ids.insert(4, HEADERS);
                ids.insert(5, 400);
            })],
            still: vec![0, 1, 2, 3],
        },
        Case {
            named: "a measured group reopened while closing",
            rows: vec![0, 1, HEADERS, 2, 3, 4, HEADERS + 1, 5, 6, 7, 8, 9, 10, 11],
            measured: true,
            scrolled: 0.0,
            changes: vec![
                (1, |ids| drop(ids.drain(2..6))),
                (5, |ids| {
                    ids.insert(2, HEADERS + 2);
                    ids.insert(3, 500);
                }),
            ],
            still: vec![0, 1],
        },
        Case {
            named: "a fold closed in a scrolled view",
            rows: rows(60),
            measured: false,
            scrolled: 300.0,
            changes: vec![(1, |ids| drop(ids.drain(18..22)))],
            still: vec![15, 16, 17],
        },
    ];
    for case in &cases {
        watched(case);
    }
}

#[test]
fn insert_above_keeps_visible_rows() {
    let mut watch = Watch::new(&rows(60), false, 300.0);
    let before = layout(&watch.frame(FRAME));
    watch
        .ui
        .update(|rows| insert_children(&mut rows.ids, 2, 100, 3));
    for frame in 0..FRAMES {
        assert_eq!(
            layout(&watch.frame(FRAME)),
            before,
            "visible content moved at frame {frame}"
        );
    }
}

const FOLDED: [u64; 4] = [100, 101, 102, 103];

const LEAP: f32 = 16.0;

fn unfold(ids: &mut Vec<u64>) {
    for (offset, id) in FOLDED.into_iter().enumerate() {
        ids.insert(3 + offset, id);
    }
}

fn fold(ids: &mut Vec<u64>) {
    ids.retain(|id| !FOLDED.contains(id));
}

#[test]
fn folding_back_mid_motion_keeps_rows_where_drawn() {
    let unfolded: Vec<u64> = [0, 1, 2].into_iter().chain(FOLDED).chain(3..12).collect();
    for (named, opened) in [
        ("closed while opening", false),
        ("reopened while closing", true),
    ] {
        let start = match opened {
            true => unfolded.clone(),
            false => rows(12),
        };
        let mut watch = Watch::new(&start, false, 0.0);
        let mut drawn = watch.frame(FRAME);
        for frame in 0..FRAMES {
            match (frame, opened) {
                (1, true) | (6, false) => watch.ui.update(|rows| fold(&mut rows.ids)),
                (1, false) | (6, true) => watch.ui.update(|rows| unfold(&mut rows.ids)),
                _ => {}
            }
            let shown = watch.frame(FRAME);
            for now in &shown {
                let Some(was) = drawn.iter().find(|was| was.id == now.id) else {
                    continue;
                };
                assert!(
                    (now.at - was.at).abs() < LEAP,
                    "{named}: row {} leapt from {} to {} at frame {frame}",
                    now.id,
                    was.at,
                    now.at
                );
            }
            drawn = shown;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Typing {
    measured: bool,
    scrolled: f32,
}

impl Typing {
    fn results(self, seed: &mut u64) -> Vec<u64> {
        let mut next = || {
            *seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *seed >> 33
        };
        let (pool, fewest, most) = match self.scrolled > 0.0 {
            true => (60, 20, 50),
            false => (24, 1, 14),
        };
        let mut results: Vec<u64> = rows(pool);
        if self.measured {
            results.extend(HEADERS..HEADERS + pool / 4);
        }
        for at in (1..results.len()).rev() {
            results.swap(at, (next() % (at as u64 + 1)) as usize);
        }
        results.truncate((fewest + next() % (most - fewest + 1)) as usize);
        results
    }
}

#[test]
fn rows_never_overlap_during_rapid_changes() {
    let typings = [false, true]
        .into_iter()
        .flat_map(|measured| [0.0, 90.0].map(|scrolled| Typing { measured, scrolled }));
    let runs = typings.flat_map(|typing| {
        (1..=100u64).flat_map(move |case| [1, 2, 3].map(|every| (typing, case, every)))
    });
    for (typing, case, every) in runs {
        let named = format!("{typing:?}, case {case}, a change every {every} frames");
        let mut seed = case;
        let mut watch = Watch::new(&typing.results(&mut seed), typing.measured, typing.scrolled);
        for frame in 0..FRAMES {
            if frame % every == 0 && frame < FRAMES / 2 {
                let ids = typing.results(&mut seed);
                watch.ui.update(|rows| rows.ids = ids);
            }
            let shown = watch.frame(FRAME);
            assert_eq!(
                overlapping(&shown),
                None,
                "{named}: rows overlap at frame {frame}: {:?}",
                layout(&shown)
            );
        }
        let settled = watch.rests();
        settled_in_order(&named, &watch.ui.state().ids, &settled);
    }
}

const FOLDER: u64 = 2;

#[derive(State)]
struct Folding {
    open: Tracked<bool>,
}

fn folding_view(folding: &Folding) -> Element {
    let ids: Rc<[u64]> = match *folding.open {
        true => [0, 1, FOLDER]
            .into_iter()
            .chain(FOLDED)
            .chain(3..12)
            .collect(),
        false => rows(12).into(),
    };
    let named = Rc::clone(&ids);
    lazy_column(ids.len(), move |slot| {
        let id = ids[slot.index];
        let row = container(spacer()).fill_width().height(Length::px(ROW));
        match id == FOLDER {
            true => row
                .on_press(|folding: &mut Folding| {
                    let open = !*folding.open;
                    folding.open.set(open);
                })
                .any(),
            false => row.on_press(|_: &mut Folding| {}).any(),
        }
    })
    .named(move |index| named[index])
    .uniform(Length::px(ROW))
    .fill()
    .build()
}

fn click(ui: &mut TypedUi<Folding>, at: Point) {
    let (button, modifiers) = (PointerButton::Primary, Modifiers::NONE);
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(Event::PointerPressed {
        position: at,
        button,
        modifiers,
    });
    ui.handle_event(Event::PointerReleased {
        position: at,
        button,
        modifiers,
    });
}

#[test]
fn folder_row_takes_clicks_while_folding() {
    let folder = Point::new(VIEW.width / 2.0, FOLDER as f32 * ROW + ROW / 2.0);
    for opened in [false, true] {
        let mut ui = TypedUi::new(
            Folding {
                open: Tracked::new(opened),
            },
            folding_view,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        let mut scene = Scene::new();
        let mut now = Instant::now();
        ui.render(Rendering::new(&mut scene).with_time(now));
        now += Duration::from_secs(1);
        ui.render(Rendering::new(&mut scene).with_time(now));

        click(&mut ui, folder);
        for _ in 0..4 {
            now += FRAME;
            ui.request_frame();
            ui.render(Rendering::new(&mut scene).with_time(now));
        }
        click(&mut ui, folder);
        assert_eq!(
            *ui.state().open,
            opened,
            "started open {opened}: second click mid-motion did not reach the folder row"
        );
    }
}
