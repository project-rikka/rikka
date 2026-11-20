use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Modifiers, Point, PointerButton, Rect, Size};
use rikka_ui::{
    Anchor, Behavior, Curve, DurationToken, Easing, Element, Event, EventCtx, Justify, Layout,
    Length, PaintCtx, Quad, Rendering, State, Tracked, TypedUi, View, Widget, component, fade,
    is_exiting, offset, pop,
};

const VIEW: Size = Size::new(400.0, 300.0);

const LIFT: f32 = 40.0;

const ENTER: Duration = Duration::from_millis(200);

const SLOW_FIRST_FRAME: Duration = Duration::from_secs(1);

struct Card;

impl Widget for Card {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(100.0), Length::px(50.0))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        ctx.draw_quad(Quad::new(ctx.bounds()).fill(Color::WHITE));
    }

    fn same(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Default, State)]
struct Shown;

fn shown(_shown: &Shown) -> impl View {
    Element::new(Card).transition(offset(0.0, LIFT).enter(Curve::Tween {
        easing: Easing::Linear,
        duration: DurationToken::Short4,
    }))
}

fn card_top<S: State>(ui: &TypedUi<S>) -> f32 {
    let dump = ui.dump();
    let line = dump
        .lines()
        .find(|line| line.contains("Card"))
        .expect("card missing from tree");
    let bounds = &line[line.find('[').expect("card has no bounds") + 1..];
    bounds
        .split_whitespace()
        .nth(1)
        .and_then(|top| top.parse().ok())
        .expect("card top not a number")
}

#[test]
fn entrance_starts_on_second_frame() {
    let first = Instant::now();
    let second = first + SLOW_FIRST_FRAME;
    let frames = [first, second, second + ENTER / 2, second + ENTER];
    for (arrives, tops) in [
        (false, [0.0, 0.0, 0.0, 0.0]),
        (true, [LIFT, LIFT, LIFT / 2.0, 0.0]),
    ] {
        let mut ui = TypedUi::new(Shown, shown, Theme::dark());
        if arrives {
            ui.arrive();
        }
        ui.set_viewport(VIEW, 1.0);
        let mut scene = Scene::new();
        for (at, top) in frames.into_iter().zip(tops) {
            ui.render(Rendering::new(&mut scene).with_time(at));
            let drawn = card_top(&ui);
            assert!(
                (drawn - top).abs() < 0.5,
                "arrival {arrives}: card at {drawn}, expected {top}, {:?} in",
                at - first
            );
        }
    }
}

const TARGET: &str = "target";

const HALFWAY: f32 = 0.96;

struct Mark {
    height: f32,
}

impl Widget for Mark {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(40.0), Length::px(self.height))
    }
}

struct Stage;

impl Widget for Stage {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }
}

#[derive(Default, State)]
struct Popping {
    target_top: f32,
}

fn popping(popping: &Popping) -> Element {
    Element::with_children(
        Stage,
        vec![
            Element::new(Mark {
                height: popping.target_top,
            }),
            Element::new(Mark { height: 20.0 }).keyed(TARGET),
            Element::new(Card)
                .transition(pop().enter(Curve::Tween {
                    easing: Easing::Linear,
                    duration: DurationToken::Short4,
                }))
                .anchored(Anchor::below(TARGET)),
        ],
    )
}

fn card_quad(ui: &TypedUi<Popping>) -> Rect {
    let dump = ui.dump();
    let line = dump
        .lines()
        .skip_while(|line| !line.contains("Card"))
        .find(|line| line.contains("· quad"))
        .expect("card painted no quad");
    let inside = &line[line.find('[').expect("quad has no rect") + 1..line.find(']').unwrap()];
    let numbers: Vec<f32> = inside
        .split([' ', 'x'])
        .filter_map(|part| part.parse().ok())
        .collect();
    Rect::new(numbers[0], numbers[1], numbers[2], numbers[3])
}

#[test]
fn pop_grows_from_anchor_edge() {
    for (target_top, pivot, resting) in [
        (
            20.0,
            Point::new(20.0, 40.0),
            Rect::new(0.0, 40.0, 100.0, 50.0),
        ),
        (
            280.0,
            Point::new(20.0, 280.0),
            Rect::new(0.0, 230.0, 100.0, 50.0),
        ),
    ] {
        let mut ui = TypedUi::new(Popping { target_top }, popping, Theme::dark());
        ui.arrive();
        ui.set_viewport(VIEW, 1.0);
        let mut scene = Scene::new();
        let first = Instant::now();
        let second = first + SLOW_FIRST_FRAME;
        ui.render(Rendering::new(&mut scene).with_time(first));
        ui.render(Rendering::new(&mut scene).with_time(second));
        ui.render(Rendering::new(&mut scene).with_time(second + ENTER / 2));

        let toward = |corner: Point| pivot + (corner - pivot) * HALFWAY;
        let origin = toward(resting.origin);
        let expected = Rect::new(
            origin.x,
            origin.y,
            resting.width() * HALFWAY,
            resting.height() * HALFWAY,
        );
        let drawn = card_quad(&ui);
        let near = [
            (drawn.x(), expected.x()),
            (drawn.y(), expected.y()),
            (drawn.width(), expected.width()),
            (drawn.height(), expected.height()),
        ]
        .into_iter()
        .all(|(drawn, target)| (drawn - target).abs() < 0.05);
        assert!(
            near,
            "card under target at {target_top} drawn at {drawn:?}, expected {expected:?}"
        );
    }
}

const BACKDROP: Point = Point::new(300.0, 100.0);

struct Backdrop;

impl Widget for Backdrop {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(200.0))
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            ctx.set_handled();
            ctx.run(|leaving: &mut Leaving| leaving.presses += 1);
        }
    }
}

struct Sheet;

impl Widget for Sheet {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(100.0), Length::px(50.0))
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::MODAL | Behavior::FOCUSABLE | Behavior::AUTOFOCUS
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            ctx.set_handled();
        }
    }
}

struct Slot;

impl Widget for Slot {
    type State = ();
}

#[derive(Default, State)]
struct Leaving {
    open: Tracked<bool>,
    #[untracked]
    presses: u32,
}

fn leaving(_: &Leaving) -> impl View {
    Element::with_children(
        Stage,
        vec![
            Element::new(Backdrop),
            component(|leaving: &Leaving| leaving, sheet),
        ],
    )
}

fn sheet(leaving: &Leaving) -> Element {
    let shown = (*leaving.open)
        .then(|| Element::with_children(Slot, vec![Element::new(Sheet)]).transition(fade()));
    Element::with_children(Slot, shown.into_iter().collect())
}

fn press_backdrop(ui: &mut TypedUi<Leaving>) -> u32 {
    let modifiers = Modifiers::NONE;
    let button = PointerButton::Primary;
    ui.handle_event(Event::PointerMoved(BACKDROP));
    ui.handle_event(Event::PointerPressed {
        position: BACKDROP,
        button,
        modifiers,
    });
    ui.handle_event(Event::PointerReleased {
        position: BACKDROP,
        button,
        modifiers,
    });
    let mut presses = 0;
    ui.update(|leaving| presses = leaving.presses);
    presses
}

#[test]
fn exiting_modal_releases_pointer_until_reopened() {
    let mut ui = TypedUi::new(Leaving::default(), leaving, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let first = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(first));
    ui.start(|leaving: &mut Leaving| leaving.open.set(true));
    ui.render(Rendering::new(&mut scene).with_time(first + SLOW_FIRST_FRAME));
    ui.render(Rendering::new(&mut scene).with_time(first + SLOW_FIRST_FRAME * 2));
    ui.start(|leaving: &mut Leaving| leaving.open.set(false));
    ui.render(Rendering::new(&mut scene).with_time(first + SLOW_FIRST_FRAME * 3));
    assert!(ui.dump().contains("Sheet"), "sheet not kept as ghost");
    assert_eq!(
        press_backdrop(&mut ui),
        1,
        "outside press did not reach backdrop"
    );

    ui.start(|leaving: &mut Leaving| leaving.open.set(true));
    ui.render(Rendering::new(&mut scene).with_time(first + SLOW_FIRST_FRAME * 3));
    assert_eq!(
        press_backdrop(&mut ui),
        1,
        "reopened sheet let an outside press through"
    );
}

struct Floor;

impl Widget for Floor {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::FILL, Length::FILL)
            .justify(Justify::End)
    }
}

struct Pile;

impl Widget for Pile {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
    }
}

const MARK: f32 = 20.0;

#[derive(State)]
struct Stacked {
    card: Tracked<bool>,
    mark: Tracked<bool>,
}

fn stacked(stacked: &Stacked) -> Element {
    let card = (*stacked.card).then(|| Element::new(Card).transition(fade()));
    let mark = (*stacked.mark).then(|| Element::new(Mark { height: MARK }));
    Element::with_children(
        Floor,
        vec![Element::with_children(
            Pile,
            card.into_iter().chain(mark).collect(),
        )],
    )
}

#[test]
fn ghost_leaves_from_where_it_was_drawn_then_rides_its_parent() {
    let mut ui = TypedUi::new(
        Stacked {
            card: Tracked::new(true),
            mark: Tracked::new(true),
        },
        stacked,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let first = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(first));
    let drawn = card_top(&ui);

    ui.start(|stacked: &mut Stacked| stacked.card.set(false));
    ui.render(Rendering::new(&mut scene).with_time(first + Duration::from_millis(1)));
    let leaving = card_top(&ui);
    assert!(
        (leaving - drawn).abs() < 0.5,
        "ghost moved from {drawn} to {leaving} as its pile shrank"
    );

    ui.start(|stacked: &mut Stacked| stacked.mark.set(false));
    ui.render(Rendering::new(&mut scene).with_time(first + Duration::from_millis(2)));
    let carried = card_top(&ui);
    assert!(
        (carried - (drawn + MARK)).abs() < 0.5,
        "ghost at {carried} after its pile moved {MARK}, expected {}",
        drawn + MARK
    );
}

const LINEAR: Curve = Curve::Tween {
    easing: Easing::Linear,
    duration: DurationToken::Short4,
};

const CARD: &str = "card";

#[derive(State)]
struct Toggled {
    open: Tracked<bool>,
    keyed: bool,
}

fn toggled(toggled: &Toggled) -> Element {
    let card = (*toggled.open && !is_exiting()).then(|| {
        let card = Element::new(Card).transition(offset(0.0, LIFT).enter(LINEAR).exit(LINEAR));
        match toggled.keyed {
            true => card.keyed(CARD),
            false => card,
        }
    });
    Element::with_children(Pile, card.into_iter().collect())
}

#[derive(Clone, Copy, Debug)]
enum Via {
    State,
    Ui,
}

fn toggle(ui: &mut TypedUi<Toggled>, via: Via, open: bool) {
    match (via, open) {
        (Via::State, open) => ui.start(move |toggled: &mut Toggled| toggled.open.set(open)),
        (Via::Ui, true) => ui.rejoin(),
        (Via::Ui, false) => ui.leave(),
    }
}

#[test]
fn reopening_mid_exit_turns_back_from_where_it_was() {
    for (keyed, via) in [(true, Via::State), (false, Via::State), (false, Via::Ui)] {
        let named = format!("keyed {keyed}, toggled via {via:?}");
        let mut ui = TypedUi::new(
            Toggled {
                open: Tracked::new(true),
                keyed,
            },
            toggled,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        let mut scene = Scene::new();
        let first = Instant::now();
        ui.render(Rendering::new(&mut scene).with_time(first));

        let closed = first + SLOW_FIRST_FRAME;
        toggle(&mut ui, via, false);
        ui.render(Rendering::new(&mut scene).with_time(closed));
        ui.render(Rendering::new(&mut scene).with_time(closed + ENTER / 2));
        assert!(
            (card_top(&ui) - LIFT / 2.0).abs() < 0.5,
            "{named}: halfway out at {}",
            card_top(&ui)
        );

        let reopened = closed + ENTER / 2;
        toggle(&mut ui, via, true);
        for (after, top) in [
            (Duration::ZERO, LIFT / 2.0),
            (ENTER / 4, LIFT / 4.0),
            (ENTER / 2, 0.0),
        ] {
            ui.render(Rendering::new(&mut scene).with_time(reopened + after));
            let dump = ui.dump();
            let cards = dump.lines().filter(|line| line.contains("Card")).count();
            assert_eq!(cards, 1, "{named}: {cards} cards {after:?} after reopening");
            assert!(
                (card_top(&ui) - top).abs() < 0.5,
                "{named}: card at {} {after:?} after reopening, expected {top}",
                card_top(&ui)
            );
        }
    }
}

const DIAL: &str = "dial";

struct Dial {
    jump: f32,
    seen: Rc<Cell<f32>>,
}

impl Widget for Dial {
    type State = Option<f32>;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(10.0), Length::px(10.0))
    }

    fn paint(&self, jumped: &mut Option<f32>, ctx: &mut PaintCtx<'_>) {
        if *jumped != Some(self.jump) {
            ctx.jump(DIAL, self.jump);
            *jumped = Some(self.jump);
        }
        self.seen.set(ctx.progress(DIAL, true, LINEAR, LINEAR));
    }
}

#[derive(State)]
struct Dialed {
    jump: Tracked<f32>,
    seen: Rc<Cell<f32>>,
}

fn dialed(dialed: &Dialed) -> Element {
    Element::new(Dial {
        jump: *dialed.jump,
        seen: Rc::clone(&dialed.seen),
    })
}

#[test]
fn progress_continues_from_a_jump() {
    let seen = Rc::new(Cell::new(f32::NAN));
    let mut ui = TypedUi::new(
        Dialed {
            jump: Tracked::new(1.0),
            seen: Rc::clone(&seen),
        },
        dialed,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let first = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(first));
    assert_eq!(seen.get(), 1.0, "progress ignored the first jump");

    let jumped = first + SLOW_FIRST_FRAME;
    ui.start(|dialed: &mut Dialed| dialed.jump.set(0.5));
    for (after, progress) in [(Duration::ZERO, 0.5), (ENTER / 4, 0.75)] {
        ui.render(Rendering::new(&mut scene).with_time(jumped + after));
        assert!(
            (seen.get() - progress).abs() < 1e-3,
            "progress at {} {after:?} after jumping to 0.5, expected {progress}",
            seen.get()
        );
    }
}
