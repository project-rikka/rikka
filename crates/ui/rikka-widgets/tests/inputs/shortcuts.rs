use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(320.0, 200.0);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Toy {
    Go,
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
enum Recorded {
    Ran(Toy),
    Held(bool),
}

#[derive(Default, State)]
struct Probe {
    log: Vec<Recorded>,
}

fn pressed(modifiers: Modifiers, key: Key) -> Option<Toy> {
    match (modifiers.control, key) {
        (true, Key::Char('g')) => Some(Toy::Go),
        (true, Key::Char('s')) => Some(Toy::Stop),
        _ => None,
    }
}

fn probing(_probe: &Probe) -> impl View {
    shortcuts(
        spacer().fill(),
        pressed,
        |toy: Toy, _probe: &Probe| toy == Toy::Go,
        |toy: Toy, probe: &mut Probe| probe.log.push(Recorded::Ran(toy)),
    )
    .standard(|standard: Command| (standard == Command::Undo).then_some(Toy::Go))
    .holding(Key::Space, |probe: &mut Probe, is_down: bool| {
        probe.log.push(Recorded::Held(is_down));
    })
}

fn key(ui: &mut TypedUi<Probe>, key: Key, modifiers: Modifiers, down: bool) {
    let (code, location) = (KeyCode::UNKNOWN, KeyLocation::Standard);
    ui.handle_event(match down {
        true => Event::KeyPressed {
            key,
            code,
            location,
            modifiers,
        },
        false => Event::KeyReleased {
            key,
            code,
            location,
            modifiers,
        },
    });
}

#[test]
fn shortcuts_run_enabled_commands_and_report_hold() {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Probe::default(), probing, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    let at = Point::new(VIEW.width / 2.0, VIEW.height / 2.0);
    ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut scene);
    let control = Modifiers {
        control: true,
        ..Modifiers::NONE
    };

    key(&mut ui, Key::Char('g'), control, true);
    key(&mut ui, Key::Char('s'), control, true);
    ui.handle_event(Event::Command(Command::Undo));
    key(&mut ui, Key::Space, Modifiers::NONE, true);
    key(&mut ui, Key::Space, Modifiers::NONE, false);

    assert_eq!(
        ui.state().log,
        vec![
            Recorded::Ran(Toy::Go),
            Recorded::Ran(Toy::Go),
            Recorded::Held(true),
            Recorded::Held(false),
        ]
    );
}
