use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::{Scene, SceneFrame};
use rikka_theme::Theme;
use rikka_types::{Color, Modifiers, Point, PointerButton, Rect, ScrollDelta, Size};
use rikka_ui::{
    Axis, Behavior, Element, Event, EventCtx, Layout, Length, PaintCtx, Quad, Rendering,
    ScrollSource, State, TextRun, TypedUi, View, Widget, can_retain, stats,
};

const VIEW: Size = Size::new(200.0, 150.0);

const ROW: f32 = 20.0;

#[derive(Widget)]
struct Probe {
    label: u32,
    painted: Rc<Cell<u32>>,
}

impl Widget for Probe {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(ROW))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        self.painted.set(self.painted.get() + 1);
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(Color::WHITE));
        ctx.draw_text(bounds, TextRun::new("row", 12.0), Color::BLACK);
    }

    fn same(&self, other: &Self) -> bool {
        self.label == other.label && Rc::ptr_eq(&self.painted, &other.painted)
    }
}

struct Column(Vec<Element>);

impl Widget for Column {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().width(Length::FILL)
    }
}

impl View for Column {
    fn build(self) -> Element {
        let Column(children) = self;
        Element::with_children(Column(Vec::new()), children)
    }
}
struct Scroller(Element);

#[derive(Default)]
struct Scrolled {
    at: f32,
}

impl Widget for Scroller {
    type State = Scrolled;

    fn layout(&self) -> Layout {
        Layout::column()
            .scrolling(Axis::Vertical)
            .size(Length::FILL, Length::FILL)
    }

    fn event(&self, state: &mut Scrolled, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::Scroll { delta, .. } = event {
            state.at = (state.at + delta.offset().y).max(0.0);
            ctx.request_paint();
            ctx.set_handled();
        }
    }

    fn behavior(&self, _state: &Scrolled) -> Behavior {
        Behavior::CLIPS
    }

    fn children_offset(&self, state: &Scrolled, _bounds: Rect, _content: Size) -> Point {
        Point::new(0.0, -state.at)
    }
}

impl View for Scroller {
    fn build(self) -> Element {
        let Scroller(content) = self;
        Element::with_children(Scroller(Element::empty()), vec![content])
    }
}

#[derive(Default, State)]
struct Rows {
    count: u32,
    changed: Option<u32>,
    painted: Rc<Cell<u32>>,
    other: Rc<Cell<u32>>,
}

fn rows(rows: &Rows) -> impl View {
    let probes = (0..rows.count)
        .map(|index| {
            let painted = match rows.changed == Some(index) {
                true => Rc::clone(&rows.other),
                false => Rc::clone(&rows.painted),
            };
            let label = match rows.changed == Some(index) {
                true => index + 1_000,
                false => index,
            };
            Probe { label, painted }.build()
        })
        .collect();
    Scroller(Column(probes).build())
}

fn open_ui(count: u32) -> (TypedUi<Rows>, Scene, Instant) {
    let mut ui = TypedUi::new(
        Rows {
            count,
            ..Rows::default()
        },
        rows,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    scene.take_frame(&mut SceneFrame::default());
    (ui, scene, now)
}

fn scroll<S: State>(ui: &mut TypedUi<S>, by: f32) {
    ui.handle_event(Event::Scroll {
        source: ScrollSource::Wheel,
        position: Point::new(VIEW.width / 2.0, VIEW.height / 2.0),
        delta: ScrollDelta::from_offset(Point::new(0.0, by)),
        modifiers: Modifiers::NONE,
    });
}

#[test]
fn unchanged_node_is_not_repainted() {
    let (mut ui, mut scene, now) = open_ui(5);
    let painted = ui.state().painted.get();
    assert_eq!(painted, 5, "rows not painted exactly once");

    ui.update(|rows| rows.changed = Some(2));
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));

    assert_eq!(ui.state().painted.get(), painted, "unchanged rows painted");
    assert_eq!(ui.state().other.get(), 1, "changed row not painted");
}

#[test]
fn scrolled_back_row_is_not_repainted() {
    let (mut ui, mut scene, now) = open_ui(40);
    let painted = ui.state().painted.get();
    let first = |ui: &TypedUi<Rows>| {
        ui.dump()
            .lines()
            .find(|line| line.contains("Probe"))
            .map(str::to_owned)
            .unwrap_or_default()
    };

    scroll(&mut ui, ROW * 20.0);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));
    assert!(first(&ui).ends_with("culled"), "first row still visible");

    scroll(&mut ui, -ROW * 20.0);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(32)));
    assert!(!first(&ui).ends_with("culled"), "first row not restored");

    let repaints = ui.state().painted.get() - painted;
    assert!(repaints < 20, "rehidden rows repainted ({repaints} paints)");
}

#[derive(Clone, Copy, Default)]
enum Reads {
    #[default]
    Viewport,
    LowerHalf,
    Level,
}

struct Reader {
    reads: Reads,
    height: f32,
    level: Rc<Cell<f32>>,
    painted: Rc<Cell<u32>>,
}

impl Widget for Reader {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(self.height))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        self.painted.set(self.painted.get() + 1);
        let bounds = ctx.bounds();
        match self.reads {
            Reads::Viewport => {
                let top = bounds.y().min(ctx.viewport().height - ROW);
                let row_rect = Rect::new(bounds.x(), top, bounds.width(), ROW);
                ctx.draw_quad(Quad::new(row_rect).fill(Color::WHITE));
            }
            Reads::LowerHalf => {
                let half = bounds.height() / 2.0;
                let lower = Rect::new(bounds.x(), bounds.y() + half, bounds.width(), half);
                ctx.clipped(lower, |ctx| {
                    ctx.draw_quad(Quad::new(lower).fill(Color::WHITE));
                });
            }
            Reads::Level => {
                let filled = Rect::new(
                    bounds.x(),
                    bounds.y(),
                    bounds.width() * self.level.get(),
                    bounds.height(),
                );
                ctx.draw_quad(Quad::new(filled).fill(Color::WHITE));
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.painted, &other.painted) && self.height == other.height
    }
}

#[derive(Default, State)]
struct Reading {
    reads: Reads,
    height: f32,
    above: u32,
    level: Rc<Cell<f32>>,
    painted: Rc<Cell<u32>>,
}

fn reading(reading: &Reading) -> impl View {
    let probes = |count: u32| {
        (0..count).map(|label| {
            Probe {
                label,
                painted: Rc::new(Cell::new(0)),
            }
            .build()
        })
    };
    let reader = Element::new(Reader {
        reads: reading.reads,
        height: reading.height,
        level: Rc::clone(&reading.level),
        painted: Rc::clone(&reading.painted),
    });
    let rows = probes(reading.above)
        .chain(std::iter::once(reader))
        .chain(probes(40))
        .collect();
    Scroller(Column(rows).build())
}

fn reads(reads: Reads, above: u32, height: f32) -> (TypedUi<Reading>, Scene, Instant) {
    let mut ui = TypedUi::new(
        Reading {
            reads,
            height,
            above,
            ..Reading::default()
        },
        reading,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    (ui, scene, now)
}

fn scrolled(ui: &mut TypedUi<Reading>, by: f32) {
    ui.handle_event(Event::Scroll {
        source: ScrollSource::Wheel,
        position: Point::new(VIEW.width / 2.0, VIEW.height / 2.0),
        delta: ScrollDelta::from_offset(Point::new(0.0, by)),
        modifiers: Modifiers::NONE,
    });
}

#[test]
fn viewport_reader_repaints_on_move() {
    let (mut ui, mut scene, now) = reads(Reads::Viewport, 2, ROW);
    let painted = ui.state().painted.get();

    scrolled(&mut ui, ROW);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));

    assert_eq!(
        ui.state().painted.get(),
        painted + 1,
        "viewport-relative node kept stale position"
    );
}

#[test]
fn clipped_node_repaints_when_revealed() {
    let lower_half_offscreen = VIEW.height - ROW / 2.0;
    let above = (lower_half_offscreen / ROW) as u32;
    let (mut ui, mut scene, now) = reads(Reads::LowerHalf, above, ROW * 2.0);
    let painted = ui.state().painted.get();

    scrolled(&mut ui, ROW * 2.0);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));

    assert_eq!(
        ui.state().painted.get(),
        painted + 1,
        "previously clipped content not drawn"
    );
}

#[test]
fn full_repaint_reaches_offscreen_rows() {
    let (mut ui, mut scene, now) = reads(Reads::Level, 0, ROW);
    scrolled(&mut ui, ROW * 30.0);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));
    let painted = ui.state().painted.get();

    ui.state().level.set(0.5);
    ui.request_frame();
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(32)));
    scrolled(&mut ui, -ROW * 30.0);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(48)));

    assert_eq!(
        ui.state().painted.get(),
        painted + 1,
        "offscreen row not repainted after full repaint"
    );
}

#[derive(Default, State)]
struct Feed {
    arrived: u32,
    painted: Rc<Cell<u32>>,
}

fn feed(feed: &Feed) -> impl View {
    let probes = (0..feed.arrived)
        .rev()
        .chain(1_000..1_004)
        .map(|label| {
            Probe {
                label,
                painted: Rc::clone(&feed.painted),
            }
            .build()
        })
        .collect();
    Column(probes)
}

#[test]
fn inserted_row_shifts_without_repaint() {
    let mut ui = TypedUi::new(Feed::default(), feed, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    let before = ui.state().painted.get();

    ui.update(|feed| feed.arrived = 1);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));

    assert_eq!(
        ui.state().painted.get() - before,
        1,
        "rows other than inserted one painted"
    );
}

struct Wrap;

impl Widget for Wrap {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().width(Length::FILL)
    }

    fn same(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Default, State)]
struct Wrapped {
    rows: u32,
    painted: Rc<Cell<u32>>,
}

fn wrapped(wrapped: &Wrapped) -> impl View {
    let rows = (0..wrapped.rows)
        .map(|label| {
            let probe = Probe {
                label,
                painted: Rc::clone(&wrapped.painted),
            };
            Element::with_children(Wrap, vec![probe.build()])
        })
        .collect();
    Column(rows)
}

#[test]
fn appended_wrapped_row_keeps_rows_above() {
    let mut ui = TypedUi::new(
        Wrapped {
            rows: 4,
            ..Wrapped::default()
        },
        wrapped,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    let before = ui.state().painted.get();

    ui.update(|wrapped| wrapped.rows = 5);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));

    assert_eq!(
        ui.state().painted.get() - before,
        1,
        "rows other than inserted one painted"
    );
}

const SHELVED: u32 = 5;

const CHANGING: u32 = 2;

#[derive(Widget)]
struct Shelf {
    painted: Rc<Cell<u32>>,
    other: Rc<Cell<u32>>,
    keeping: Rc<RefCell<Vec<bool>>>,
}

#[derive(Default)]
struct Shelved {
    changed: bool,
}

impl Widget for Shelf {
    type State = Shelved;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, state: &mut Shelved, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::Scroll { .. } = event {
            state.changed = true;
            ctx.handle().rebuild();
            ctx.set_handled();
        }
    }

    fn build(&self, state: &mut Shelved) -> Vec<Element> {
        self.keeping.borrow_mut().push(can_retain());
        (0..SHELVED)
            .map(|index| {
                let changing = state.changed && index == CHANGING;
                if can_retain() && !changing {
                    return Element::retained(index);
                }
                let (label, painted) = match changing {
                    true => (index + 1_000, &self.other),
                    false => (index, &self.painted),
                };
                Probe {
                    label,
                    painted: Rc::clone(painted),
                }
                .build()
                .keyed(index)
            })
            .collect()
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.painted, &other.painted)
            && Rc::ptr_eq(&self.other, &other.other)
            && Rc::ptr_eq(&self.keeping, &other.keeping)
    }
}

#[derive(Default, State)]
struct Shelving {
    painted: Rc<Cell<u32>>,
    other: Rc<Cell<u32>>,
    keeping: Rc<RefCell<Vec<bool>>>,
}

fn shelving(shelving: &Shelving) -> impl View {
    Shelf {
        painted: Rc::clone(&shelving.painted),
        other: Rc::clone(&shelving.other),
        keeping: Rc::clone(&shelving.keeping),
    }
}

#[test]
fn retained_child_repaints_on_theme_change() {
    let mut ui = TypedUi::new(Shelving::default(), shelving, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    let painted = ui.state().painted.get();
    assert_eq!(painted, SHELVED, "rows not painted exactly once");
    let rows = |ui: &TypedUi<Shelving>| ui.dump().matches("Probe").count();

    let before = stats::snapshot();
    scroll_shelf(&mut ui);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));
    let spent = stats::snapshot().since(before);

    assert_eq!(
        spent.retained,
        u64::from(SHELVED - 1),
        "unchanged rows not kept"
    );
    assert_eq!(spent.reconciled, 1, "unchanged rows reconciled");
    assert_eq!(ui.state().painted.get(), painted, "kept rows painted");
    assert_eq!(ui.state().other.get(), 1, "changed row not painted");
    assert_eq!(rows(&ui), SHELVED as usize, "rows missing");

    ui.set_theme(Theme::light());
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(32)));

    assert_eq!(
        ui.state().painted.get(),
        painted + SHELVED - 1,
        "kept rows rebuilt on theme change"
    );
    assert_eq!(ui.state().other.get(), 2, "changed row not repainted");
    assert_eq!(rows(&ui), SHELVED as usize, "rows missing");
    assert_eq!(
        *ui.state().keeping.borrow(),
        [false, true, false],
        "children kept across theme change"
    );
}

fn scroll_shelf(ui: &mut TypedUi<Shelving>) {
    ui.handle_event(Event::Scroll {
        source: ScrollSource::Wheel,
        position: Point::new(VIEW.width / 2.0, ROW * (CHANGING as f32 + 0.5)),
        delta: ScrollDelta::from_offset(Point::new(0.0, ROW)),
        modifiers: Modifiers::NONE,
    });
}

type Taps = Rc<RefCell<Vec<(u32, bool)>>>;

#[derive(Widget)]
struct Tappable {
    label: u32,
    taps: Taps,
}

#[derive(Default)]
struct Kept {
    bounds: Rect,
}

impl Widget for Tappable {
    type State = Kept;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(ROW))
    }

    fn paint(&self, state: &mut Kept, ctx: &mut PaintCtx<'_>) {
        state.bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(ctx.bounds()).fill(Color::WHITE));
    }

    fn relocated(&self, state: &mut Kept, by: Point) {
        state.bounds = state.bounds.translate(by);
    }

    fn event(&self, state: &mut Kept, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            self.taps
                .borrow_mut()
                .push((self.label, state.bounds == ctx.bounds()));
            ctx.set_handled();
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.label == other.label && Rc::ptr_eq(&self.taps, &other.taps)
    }
}

#[derive(Default, State)]
struct Tapping {
    taps: Taps,
}

const TAPPED: u32 = 200;

fn tapping(tapping: &Tapping) -> impl View {
    let rows = (0..TAPPED)
        .map(|label| {
            Tappable {
                label,
                taps: Rc::clone(&tapping.taps),
            }
            .build()
        })
        .collect();
    Scroller(Column(rows).build())
}

#[test]
fn scrolled_rows_take_presses_without_revisits() {
    let mut ui = TypedUi::new(Tapping::default(), tapping, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let mut now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    let pressed = Point::new(VIEW.width / 2.0, 70.0);
    let mut at = 0.0f32;
    for (by, revisits) in [
        (3.0, Some(4)),
        (5.0, Some(4)),
        (17.0, Some(4)),
        (400.0, None),
        (-395.0, None),
        (2.0, Some(4)),
        (1_200.0, None),
        (6.0, Some(4)),
    ] {
        let before = stats::snapshot();
        scroll(&mut ui, by);
        now += Duration::from_millis(16);
        ui.render(Rendering::new(&mut scene).with_time(now));
        let visited = stats::snapshot().since(before).visited;
        at = (at + by).max(0.0);
        if let Some(most) = revisits {
            assert!(
                visited <= most,
                "scrolling by {by} revisited {visited} nodes"
            );
        }
        for event in [
            Event::PointerPressed {
                position: pressed,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            Event::PointerReleased {
                position: pressed,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
        ] {
            ui.handle_event(event);
        }
        let under = ((pressed.y + at) / ROW) as u32;
        assert_eq!(
            ui.state().taps.borrow_mut().pop(),
            Some((under, true)),
            "press after scrolling to {at} missed its row or its kept place"
        );
    }
}
