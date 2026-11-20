use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::{
    Behavior, Command, Element, Event, EventCtx, Layout, Length, PaintCtx, Rendering, State,
    TypedUi, Widget,
};

const VIEW: Size = Size::new(200.0, 100.0);
const FIELD: Point = Point::new(50.0, 50.0);

#[derive(Default, State)]
struct Recorded {
    commands: Vec<(&'static str, Command)>,
    keys: Vec<&'static str>,
}

struct Pane {
    takes: &'static [Command],
}

impl Widget for Pane {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(VIEW.width), Length::px(VIEW.height))
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::Command(command) = *event
            && self.takes.contains(&command)
        {
            ctx.set_handled();
            ctx.run(move |recorded: &mut Recorded| recorded.commands.push(("pane", command)));
        }
    }
}

struct Field {
    copies: bool,
    takes_chord: bool,
}

impl Widget for Field {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(100.0), Length::px(VIEW.height))
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::KeyPressed {
                key: Key::Char('c'),
                modifiers,
                ..
            } if self.takes_chord && modifiers == Modifiers::CONTROL => {
                ctx.set_handled();
                ctx.run(|recorded: &mut Recorded| recorded.keys.push("field"));
            }
            Event::Command(Command::Copy) if self.copies => {
                ctx.set_handled();
                ctx.run(|recorded: &mut Recorded| recorded.commands.push(("field", Command::Copy)));
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::FOCUSABLE
    }
}

struct Bare;

impl Widget for Bare {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(100.0), Length::px(VIEW.height))
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}
}

fn ui(takes: &'static [Command], copies: bool, takes_chord: bool) -> TypedUi<Recorded> {
    let view = move |_: &Recorded| {
        Element::with_children(
            Pane { takes },
            vec![
                Element::new(Field {
                    copies,
                    takes_chord,
                }),
                Element::new(Bare),
            ],
        )
    };
    let mut ui = TypedUi::new(Recorded::default(), view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(Rendering::new(&mut Scene::new()).with_time(Instant::now()));
    ui
}

fn click(ui: &mut TypedUi<Recorded>, position: Point, button: PointerButton) {
    let modifiers = Modifiers::NONE;
    ui.handle_event(Event::PointerMoved(position));
    ui.handle_event(Event::PointerPressed {
        position,
        button,
        modifiers,
    });
    ui.handle_event(Event::PointerReleased {
        position,
        button,
        modifiers,
    });
}

fn press_key(ui: &mut TypedUi<Recorded>, key: Key, modifiers: Modifiers) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers,
    });
}

fn recorded(ui: &mut TypedUi<Recorded>) -> (Vec<(&'static str, Command)>, Vec<&'static str>) {
    let mut recorded = (Vec::new(), Vec::new());
    ui.update(|state| recorded = (state.commands.clone(), state.keys.clone()));
    recorded
}

#[test]
fn unhandled_chord_bubbles_as_command() {
    let mut ui = ui(&[Command::Copy], false, false);
    click(&mut ui, FIELD, PointerButton::Primary);

    press_key(&mut ui, Key::Char('c'), Modifiers::CONTROL);

    assert_eq!(recorded(&mut ui).0, [("pane", Command::Copy)]);
}

#[test]
fn focused_widget_receives_command_first() {
    let mut ui = ui(&[Command::Copy], true, false);
    click(&mut ui, FIELD, PointerButton::Primary);

    press_key(&mut ui, Key::Insert, Modifiers::CONTROL);

    assert_eq!(recorded(&mut ui).0, [("field", Command::Copy)]);
}

#[test]
fn raw_chord_handler_suppresses_command() {
    let mut ui = ui(&[Command::Copy], true, true);
    click(&mut ui, FIELD, PointerButton::Primary);

    press_key(&mut ui, Key::Char('c'), Modifiers::CONTROL);

    let (commands, keys) = recorded(&mut ui);
    assert!(commands.is_empty());
    assert_eq!(keys, ["field"]);
}

#[test]
fn unclaimed_command_reaches_host() {
    let mut ui = ui(&[], false, false);
    click(&mut ui, FIELD, PointerButton::Primary);

    press_key(&mut ui, Key::Char('w'), Modifiers::CONTROL);
    assert_eq!(ui.take_unclaimed_command(), Some(Command::Close));

    press_key(&mut ui, Key::Char('k'), Modifiers::CONTROL);
    assert_eq!(ui.take_unclaimed_command(), None);
}
