use std::time::{Duration, Instant};

use rikka_input::Chord;
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Rect, Size};
use rikka_ui::{
    Behavior, Element, Event, EventCtx, Layout, Length, Rendering, State, Tooltip, TooltipSide,
    TypedUi, View, Widget,
};

const VIEW: Size = Size::new(400.0, 300.0);
const BRUSH: Rect = Rect::new(20.0, 20.0, 40.0, 40.0);
const ERASER: Rect = Rect::new(70.0, 20.0, 40.0, 40.0);
const FLOOR: Rect = Rect::new(20.0, 270.0, 40.0, 30.0);
const EDGE: Rect = Rect::new(360.0, 120.0, 40.0, 40.0);
const HALVES: Rect = Rect::new(150.0, 20.0, 80.0, 40.0);
const SETTLED: [u64; 4] = [600, 700, 900, 1300];

struct Spot {
    frame: Rect,
}

impl Widget for Spot {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(
                Length::px(self.frame.size.width),
                Length::px(self.frame.size.height),
            )
            .placed_at(self.frame.origin)
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::FOCUSABLE
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            ctx.set_handled();
        }
    }
}

fn spot(frame: Rect) -> Element {
    Element::new(Spot { frame })
}

struct Halves;

impl Widget for Halves {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(
                Length::px(HALVES.size.width),
                Length::px(HALVES.size.height),
            )
            .placed_at(HALVES.origin)
    }

    fn tooltip_at(&self, (): &(), bounds: Rect, point: Point) -> Option<(Tooltip, Rect)> {
        let half = bounds.width() / 2.0;
        let (label, x) = match point.x < bounds.x() + half {
            true => ("Left", bounds.x()),
            false => ("Right", bounds.x() + half),
        };
        Some((
            Tooltip::new(label),
            Rect::new(x, bounds.y(), half, bounds.height()),
        ))
    }
}

#[derive(Default, State)]
struct Board {
    shifted: bool,
}

fn view(board: &Board) -> Element {
    let whole = Rect::from_origin_size(Point::ZERO, VIEW);
    let (brush, eraser) = match board.shifted {
        true => (ERASER, BRUSH),
        false => (BRUSH, ERASER),
    };
    Element::with_children(
        Spot { frame: whole },
        vec![
            spot(brush).tooltip(
                Tooltip::new("Brush").shortcut(Chord::new(Modifiers::NONE, Key::Char('b'))),
            ),
            spot(eraser).tooltip("Eraser"),
            spot(FLOOR).tooltip("Floor"),
            spot(EDGE).tooltip(Tooltip::new("Edge").side(TooltipSide::After)),
            Element::new(Halves),
        ],
    )
}

struct Session {
    ui: TypedUi<Board>,
    scene: Scene,
    last: Instant,
}

impl Session {
    fn open() -> Self {
        let mut ui = TypedUi::new(Board::default(), view, Theme::dark());
        let mut scene = Scene::new();
        let last = Instant::now();
        ui.set_viewport(VIEW, 1.0);
        ui.render(Rendering::new(&mut scene).with_time(last));
        Self { ui, scene, last }
    }

    fn point(&mut self, at: Point) -> Instant {
        self.ui.handle_event(Event::PointerMoved(at));
        Instant::now()
    }

    fn press(&mut self, at: Point) {
        let modifiers = Modifiers::NONE;
        let button = PointerButton::Primary;
        self.ui.handle_event(Event::PointerPressed {
            position: at,
            button,
            modifiers,
        });
        self.ui.handle_event(Event::PointerReleased {
            position: at,
            button,
            modifiers,
        });
    }

    fn key(&mut self, key: Key) {
        self.ui.handle_event(Event::KeyPressed {
            key,
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers: Modifiers::NONE,
        });
    }

    fn render_at(&mut self, at: Instant) {
        self.last = self.last.max(at);
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.last));
    }

    fn settle(&mut self) {
        let from = self.last.max(Instant::now());
        for millis in SETTLED {
            self.render_at(from + Duration::from_millis(millis));
        }
    }

    fn shown(&self, label: &str) -> Option<Rect> {
        let quoted = format!("{label:?}");
        self.ui.dump().lines().find_map(|line| {
            let drawn = line.trim().strip_prefix("· text [")?;
            let (rect, content) = drawn.split_once("] ")?;
            if content.trim() != quoted {
                return None;
            }
            let numbers: Vec<f32> = rect
                .split([' ', 'x'])
                .map(|number| number.parse().expect("number in dump"))
                .collect();
            Some(Rect::new(numbers[0], numbers[1], numbers[2], numbers[3]))
        })
    }
}

#[test]
fn tooltip_shows_after_hover_delay() {
    let mut session = Session::open();
    let at = session.point(BRUSH.center());
    session.render_at(at);
    assert!(session.shown("Brush").is_none());
    let due = session.ui.next_wake().expect("tooltip not delayed");
    assert!(due > at + Duration::from_millis(300));

    session.render_at(at + Duration::from_millis(300));
    assert!(session.shown("Brush").is_none());

    session.settle();
    let label = session.shown("Brush").expect("tooltip label missing");
    assert!(session.shown("B").is_some(), "shortcut not shown");
    assert!(label.y() > BRUSH.max_y(), "tooltip not below control");
}

#[test]
fn press_hides_tooltip_until_reentry() {
    let mut session = Session::open();
    session.point(BRUSH.center());
    session.settle();
    assert!(session.shown("Brush").is_some());

    session.press(BRUSH.center());
    session.point(Point::new(BRUSH.center().x + 2.0, BRUSH.center().y));
    session.settle();
    assert!(
        session.shown("Brush").is_none(),
        "tooltip still shown after press"
    );

    session.point(Point::new(200.0, 150.0));
    session.point(BRUSH.center());
    session.settle();
    assert!(
        session.shown("Brush").is_some(),
        "tooltip not shown after reentry"
    );
}

#[test]
fn key_press_hides_tooltip() {
    let mut session = Session::open();
    session.point(BRUSH.center());
    session.settle();
    assert!(session.shown("Brush").is_some());

    session.key(Key::Char('x'));
    session.settle();
    assert!(session.shown("Brush").is_none());
}

#[test]
fn visible_tooltip_switches_immediately() {
    let mut session = Session::open();
    session.point(BRUSH.center());
    session.settle();

    session.point(Point::new(65.0, 40.0));
    let at = session.point(ERASER.center());
    let due = session.ui.next_wake().expect("next tooltip not scheduled");
    assert!(due <= at, "next tooltip not immediate");
    session.render_at(at);
    assert!(session.shown("Eraser").is_some());
    assert!(session.shown("Brush").is_none());
}

#[test]
fn tooltips_flip_at_window_edges() {
    let mut session = Session::open();
    session.point(FLOOR.center());
    session.settle();
    let floor = session.shown("Floor").expect("floor tooltip missing");
    assert!(floor.max_y() < FLOOR.y(), "tooltip did not flip above");

    session.point(EDGE.center());
    session.settle();
    let edge = session.shown("Edge").expect("edge tooltip missing");
    assert!(edge.max_x() < EDGE.x(), "tooltip did not flip before");
    assert!(edge.x() >= 0.0);
}

#[test]
fn keyboard_focus_shows_tooltip() {
    let mut session = Session::open();
    session.key(Key::Tab);
    session.key(Key::Tab);
    session.settle();
    assert!(session.shown("Brush").is_some());
}

#[test]
fn widget_reports_tooltip_per_part() {
    let mut session = Session::open();
    session.point(Point::new(HALVES.x() + 10.0, HALVES.center().y));
    session.settle();
    let left = session.shown("Left").expect("left part tooltip missing");

    session.point(Point::new(HALVES.max_x() - 10.0, HALVES.center().y));
    session.settle();
    let right = session.shown("Right").expect("right part tooltip missing");
    assert!(session.shown("Left").is_none());
    assert!(
        right.center().x > left.center().x,
        "tooltip not under its part"
    );
}

#[test]
fn tooltip_waits_for_pointer_motion() {
    let mut session = Session::open();
    session.point(BRUSH.center());
    session.ui.update(|board| board.shifted = true);
    session.settle();
    assert!(
        session.shown("Eraser").is_none(),
        "tooltip shown without pointer motion"
    );

    session.point(Point::new(BRUSH.center().x + 2.0, BRUSH.center().y));
    session.settle();
    assert!(session.shown("Eraser").is_some());
}
