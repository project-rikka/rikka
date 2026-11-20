use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(400.0, 300.0);
const CARD: f32 = 60.0;
const FRAME: Duration = Duration::from_millis(16);
const SETTLED: Duration = Duration::from_secs(2);
const STEP: f32 = 10.0;
const HOME: Point = Point::new(100.0, CARD / 2.0);

#[derive(State)]
struct Deck {
    shown: bool,
    arriving: bool,
    gone: bool,
    dismissed: usize,
    pressed: usize,
}

impl Deck {
    fn new(shown: bool) -> Deck {
        Deck {
            shown,
            arriving: false,
            gone: false,
            dismissed: 0,
            pressed: 0,
        }
    }
}

fn deck(deck: &Deck) -> Element {
    let card = deck.shown.then(|| {
        let face = container(spacer().height(Length::px(CARD)).fill_width())
            .on_press(|deck: &mut Deck| deck.pressed += 1)
            .fill_width();
        dismissible(face)
            .arriving(deck.arriving)
            .dismissed(deck.gone)
            .on_dismiss(|deck: &mut Deck| {
                deck.dismissed += 1;
                deck.gone = true;
            })
            .fill_width()
            .build()
            .keyed(1_u64)
    });
    column! {
        if let Some(card) = card {
            card;
        }
        spacer().height(Length::px(CARD)).fill_width();
    }
    .fill_width()
    .align_self(Align::Start)
    .build()
}

fn slot(ui: &TypedUi<Deck>) -> f32 {
    ui.content_size().height - CARD
}

fn render(ui: &mut TypedUi<Deck>, scene: &mut Scene, at: Instant) {
    ui.render(Rendering::new(scene).with_time(at));
}

fn opened(shown: bool) -> (TypedUi<Deck>, Scene, Instant) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Deck::new(shown), deck, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let start = Instant::now();
    render(&mut ui, &mut scene, start);
    (ui, scene, start)
}

fn press(ui: &mut TypedUi<Deck>, at: Point) {
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
}

fn release(ui: &mut TypedUi<Deck>, at: Point) {
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
}

#[derive(Clone, Copy, Debug)]
enum Gesture {
    Drag(f32),
    Stroke(f32),
}

fn perform(ui: &mut TypedUi<Deck>, gesture: Gesture) {
    match gesture {
        Gesture::Drag(distance) => {
            press(ui, HOME);
            let mut travelled = 0.0;
            while travelled < distance {
                travelled = (travelled + STEP).min(distance);
                ui.handle_event(Event::PointerMoved(HOME + Point::new(travelled, 0.0)));
            }
            release(ui, HOME + Point::new(distance, 0.0));
        }
        Gesture::Stroke(distance) => {
            let stroke = |delta: f32, phase: ScrollPhase| Event::Scroll {
                position: HOME,
                delta: ScrollDelta::from_content(Point::new(delta, 0.0)),
                modifiers: Modifiers::NONE,
                source: ScrollSource::Continuous(phase),
            };
            ui.handle_event(stroke(distance / 2.0, ScrollPhase::Began));
            ui.handle_event(stroke(distance / 2.0, ScrollPhase::Moved));
            ui.handle_event(stroke(0.0, ScrollPhase::Ended));
        }
    }
}

fn play(ui: &mut TypedUi<Deck>, scene: &mut Scene, from: Instant, until: Instant) {
    let mut at = from;
    while at < until {
        at += FRAME;
        render(ui, scene, at);
    }
}

#[test]
fn swipes_dismiss_past_the_threshold_and_never_press() {
    let cases = [
        (Gesture::Drag(0.0), 0, 1),
        (Gesture::Drag(60.0), 0, 0),
        (Gesture::Drag(250.0), 1, 0),
        (Gesture::Stroke(60.0), 0, 0),
        (Gesture::Stroke(250.0), 1, 0),
    ];
    for (gesture, dismissed, pressed) in cases {
        let (mut ui, mut scene, start) = opened(true);
        perform(&mut ui, gesture);
        play(&mut ui, &mut scene, start, start + SETTLED);
        let deck = ui.state();
        assert_eq!(deck.dismissed, dismissed, "{gesture:?} dismissals");
        assert_eq!(deck.pressed, pressed, "{gesture:?} presses");
        let left = match dismissed {
            0 => CARD,
            _ => 0.0,
        };
        assert_eq!(slot(&ui), left, "{gesture:?} slot");
    }
}

#[test]
fn a_dismissed_card_slides_out_before_its_slot_closes() {
    let (mut ui, mut scene, start) = opened(true);
    ui.update(|deck| deck.gone = true);
    let slid = start + motion::SLIDE_OUT.duration().expect("a tween").duration();
    let mut at = start;
    while at + FRAME <= start + Dismissible::DEPARTURE {
        at += FRAME;
        render(&mut ui, &mut scene, at);
        if at <= slid {
            assert_eq!(slot(&ui), CARD, "the slot closed during the slide");
        }
    }
    assert_eq!(slot(&ui), 0.0, "the slot outlived the departure");
}

#[test]
fn an_arriving_card_opens_its_room_and_slides_home() {
    let (mut ui, mut scene, start) = opened(false);
    ui.update(|deck| {
        deck.shown = true;
        deck.arriving = true;
    });
    let mut at = start + FRAME;
    render(&mut ui, &mut scene, at);
    assert_eq!(slot(&ui), 0.0, "the room opened at once");
    press(&mut ui, HOME);
    release(&mut ui, HOME);
    assert_eq!(ui.state().pressed, 0, "the card appeared at home");
    let mut room = 0.0;
    while at < start + SETTLED {
        at += FRAME;
        render(&mut ui, &mut scene, at);
        let opening = slot(&ui);
        assert!(opening >= room, "the room shrank from {room} to {opening}");
        room = opening;
    }
    assert_eq!(room, CARD, "the room never opened");
    press(&mut ui, HOME);
    release(&mut ui, HOME);
    assert_eq!(ui.state().pressed, 1, "the arrived card ignores presses");
}
