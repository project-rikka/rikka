use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Rect, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(600.0, 600.0);
const EDGE: f32 = 16.0;
const WIDTH: f32 = 264.0;
const SQUARE: f32 = 168.0;

#[derive(State)]
struct Probe {
    color: Color,
    alpha: bool,
    enabled: bool,
    changes: Vec<Color>,
    commits: Vec<Color>,
}

impl Probe {
    fn new(color: Color) -> Self {
        Self {
            color,
            alpha: true,
            enabled: true,
            changes: Vec::new(),
            commits: Vec::new(),
        }
    }

    fn changed(&mut self, color: Color) {
        self.color = color;
        self.changes.push(color);
    }

    fn committed(&mut self, color: Color) {
        self.commits.push(color);
    }
}

fn picking(probe: &Probe) -> impl View {
    column! {
        color_picker(probe.color)
            .alpha(probe.alpha)
            .on_change(Probe::changed)
            .on_commit(Probe::committed)
    }
    .padding(Space::from(EDGE))
    .fill()
}

fn ui_of(probe: Probe, view: fn(&Probe) -> Element) -> (TypedUi<Probe>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(probe, view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    ui.render(&mut scene);
    (ui, scene)
}

fn picker(color: Color) -> (TypedUi<Probe>, Scene) {
    ui_of(Probe::new(color), |probe| picking(probe).build())
}

fn pointer(ui: &mut TypedUi<Probe>, event: Event) {
    ui.handle_event(event);
}

fn down(ui: &mut TypedUi<Probe>, scene: &mut Scene, at: Point) {
    pointer(ui, Event::PointerMoved(at));
    pointer(
        ui,
        Event::PointerPressed {
            position: at,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        },
    );
    ui.render(scene);
}

fn up(ui: &mut TypedUi<Probe>, scene: &mut Scene, at: Point) {
    pointer(
        ui,
        Event::PointerReleased {
            position: at,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        },
    );
    ui.render(scene);
}

fn click(ui: &mut TypedUi<Probe>, scene: &mut Scene, at: Point) {
    down(ui, scene, at);
    up(ui, scene, at);
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

const CONTROL: Modifiers = Modifiers {
    control: true,
    ..Modifiers::NONE
};

fn retype(ui: &mut TypedUi<Probe>, scene: &mut Scene, at: Point, typed: &str) {
    click(ui, scene, at);
    chord(ui, scene, Key::Char('a'), CONTROL);
    for letter in typed.chars() {
        chord(ui, scene, Key::Char(letter), Modifiers::NONE);
    }
    chord(ui, scene, Key::Enter, Modifiers::NONE);
}

fn rects(ui: &TypedUi<Probe>, name: &str) -> Vec<Rect> {
    ui.dump()
        .lines()
        .filter(|line| line.trim_start().starts_with(&format!("{name} [")))
        .filter_map(|line| {
            let inside = line.split('[').nth(1)?.split(']').next()?;
            let numbers: Vec<f32> = inside
                .split([' ', 'x'])
                .filter_map(|part| part.parse().ok())
                .collect();
            Some(Rect::new(numbers[0], numbers[1], numbers[2], numbers[3]))
        })
        .collect()
}

fn near(a: Color, b: Color) -> bool {
    a.to_rgba8() == b.to_rgba8()
}

fn square_at(origin: Point, saturation: f32, value: f32) -> Point {
    Point::new(
        origin.x + WIDTH * saturation,
        origin.y + SQUARE * (1.0 - value),
    )
}

const ORIGIN: Point = Point::new(EDGE, EDGE);

#[test]
fn hue_survives_black() {
    let blue = Color::rgb(0.0, 0.0, 1.0);
    let (mut ui, mut scene) = picker(blue);

    let bottom = square_at(ORIGIN, 0.99, 0.0);
    click(&mut ui, &mut scene, bottom);
    assert!(near(ui.state().color, Color::BLACK));

    click(&mut ui, &mut scene, square_at(ORIGIN, 0.999, 1.0));
    assert!(
        near(ui.state().color, blue),
        "came back as {:?}",
        ui.state().color
    );
}

#[test]
fn hex_entry_round_trips() {
    let (mut ui, mut scene) = picker(Color::rgb(1.0, 0.0, 0.0));
    let hex = rects(&ui, "TextField")[0].center();

    retype(&mut ui, &mut scene, hex, "#00FF0080");
    assert_eq!(ui.state().color.to_rgba8(), [0, 255, 0, 128]);
    assert_eq!(ui.state().commits.len(), 1);
    ui.render(&mut scene);
    assert!(ui.dump().contains("\"#00FF0080\""), "{}", ui.dump());

    retype(&mut ui, &mut scene, hex, "nonsense");
    assert_eq!(ui.state().commits.len(), 1);
    assert!(ui.dump().contains("\"#00FF0080\""), "{}", ui.dump());
}
