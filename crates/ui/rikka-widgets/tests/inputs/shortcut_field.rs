use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(360.0, 160.0);

const SUPER: Modifiers = Modifiers {
    logo: true,
    ..Modifiers::NONE
};

#[derive(Clone, Debug, PartialEq)]
enum Recorded {
    Record,
    Captured(Modifiers, Key),
    Cancelled,
}

#[derive(Default, State)]
struct Probe {
    recording: bool,
    log: Vec<Recorded>,
}

fn probing(probe: &Probe) -> impl View {
    column! {
        shortcut_field("Super+Q")
            .prompt("…")
            .recording(probe.recording)
            .on_record(|probe: &mut Probe| {
                probe.recording = true;
                probe.log.push(Recorded::Record);
            })
            .on_capture(|probe: &mut Probe, modifiers: Modifiers, key: Key| {
                probe.recording = false;
                probe.log.push(Recorded::Captured(modifiers, key));
            })
            .on_cancel(|probe: &mut Probe| {
                probe.recording = false;
                probe.log.push(Recorded::Cancelled);
            })
            .width(Length::px(200.0))
    }
    .align_items(Align::Center)
    .justify(Justify::Center)
    .padding(Space::ROOMY)
    .fill()
}

fn probe() -> (TypedUi<Probe>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Probe::default(), probing, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

fn center() -> Point {
    Point::new(VIEW.width / 2.0, VIEW.height / 2.0)
}

fn click(ui: &mut TypedUi<Probe>, scene: &mut Scene) {
    let at = center();
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(scene);
}

fn press(ui: &mut TypedUi<Probe>, key: Key, modifiers: Modifiers) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers,
    });
}

#[test]
fn pressed_chord_is_reported() {
    let (mut ui, mut scene) = probe();
    click(&mut ui, &mut scene);

    press(&mut ui, Key::MetaLeft, SUPER);
    assert_eq!(
        ui.state().log,
        vec![Recorded::Record],
        "lone Super reported as chord"
    );

    press(&mut ui, Key::Char('q'), SUPER);
    ui.render(&mut scene);
    assert_eq!(
        ui.state().log,
        vec![Recorded::Record, Recorded::Captured(SUPER, Key::Char('q'))]
    );
    assert!(!ui.grabs_keys(), "hold not reported after recording");
}
