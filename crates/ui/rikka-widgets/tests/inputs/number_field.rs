use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(400.0, 240.0);
const FIELD: Point = Point::new(50.0, 34.0);

#[derive(State)]
struct Probe {
    value: f32,
    enabled: bool,
    changes: Vec<f32>,
    other: TextDocument,
}

fn view(probe: &Probe) -> impl View {
    column! {
        number_field(probe.value)
            .range(0.0..=100.0)
            .decimals(1)
            .unit("%")
            .enabled(probe.enabled)
            .on_commit(|probe: &mut Probe, value: f32| {
                probe.value = value;
                probe.changes.push(value);
            });
        text_field(&probe.other).on_change(|| {}).width(Length::px(200.0))
    }
    .gap(Space::from(16.0))
    .padding(Space::from(16.0))
    .fill()
}

fn field(value: f32) -> (TypedUi<Probe>, Scene) {
    let mut scene = Scene::new();
    let probe = Probe {
        value,
        enabled: true,
        changes: Vec::new(),
        other: TextDocument::new(""),
    };
    let mut ui = TypedUi::new(probe, view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

fn click(ui: &mut TypedUi<Probe>, scene: &mut Scene, at: Point) {
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

fn chord(ui: &mut TypedUi<Probe>, scene: &mut Scene, key: Key, modifiers: Modifiers) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers,
    });
    ui.render(scene);
}

fn press(ui: &mut TypedUi<Probe>, scene: &mut Scene, key: Key) {
    chord(ui, scene, key, Modifiers::NONE);
}

const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};

const CONTROL: Modifiers = Modifiers {
    control: true,
    ..Modifiers::NONE
};

fn retype(ui: &mut TypedUi<Probe>, scene: &mut Scene, typed: &str) {
    click(ui, scene, FIELD);
    chord(ui, scene, Key::Char('a'), CONTROL);
    for letter in typed.chars() {
        press(ui, scene, Key::Char(letter));
    }
}

fn shows(ui: &TypedUi<Probe>, text: &str) -> bool {
    ui.dump().contains(&format!("\"{text}\""))
}

#[test]
fn enter_commits_clamped_value() {
    let (mut ui, mut scene) = field(10.0);
    retype(&mut ui, &mut scene, "250");
    assert!(ui.state().changes.is_empty(), "value changed before enter");

    press(&mut ui, &mut scene, Key::Enter);
    assert_eq!(ui.state().changes, vec![100.0]);
    assert!(shows(&ui, "100.0"), "{}", ui.dump());
}

#[test]
fn invalid_text_reverts() {
    let (mut ui, mut scene) = field(10.0);
    retype(&mut ui, &mut scene, "abc");
    press(&mut ui, &mut scene, Key::Enter);

    assert!(ui.state().changes.is_empty());
    assert!(shows(&ui, "10.0"), "{}", ui.dump());
}

#[test]
fn arrow_and_page_keys_step_within_range() {
    let (mut ui, mut scene) = field(10.0);
    click(&mut ui, &mut scene, FIELD);

    press(&mut ui, &mut scene, Key::ArrowUp);
    assert_eq!(ui.state().value, 11.0);
    chord(&mut ui, &mut scene, Key::ArrowUp, SHIFT);
    assert_eq!(ui.state().value, 21.0);
    press(&mut ui, &mut scene, Key::PageDown);
    assert_eq!(ui.state().value, 11.0);
    press(&mut ui, &mut scene, Key::ArrowDown);
    assert_eq!(ui.state().value, 10.0);
    for _ in 0..3 {
        chord(&mut ui, &mut scene, Key::ArrowDown, SHIFT);
    }
    assert_eq!(ui.state().value, 0.0);
    assert_eq!(ui.state().changes.last(), Some(&0.0));
    assert!(shows(&ui, "0.0"));
}
