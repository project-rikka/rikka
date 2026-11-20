use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(640.0, 400.0);
const EDGE: f32 = 16.0;
const PADDING: f32 = 5.0;
const MENU_WIDTH: f32 = 200.0;
const SEPARATOR: f32 = 11.0;

#[derive(State)]
struct Probe {
    open: bool,
    ran: Vec<&'static str>,
    dismissals: u32,
}

impl Probe {
    fn new() -> Self {
        Self {
            open: true,
            ran: Vec::new(),
            dismissals: 0,
        }
    }
}

fn ran(name: &'static str) -> impl Fn(&mut Probe) {
    move |probe: &mut Probe| probe.ran.push(name)
}

fn effects() -> Vec<MenuItem> {
    vec![
        menu_item("Repeat").on_press(ran("Repeat")),
        menu_separator(),
        menu_item("Blurs").items([
            menu_item("Gaussian").on_press(ran("Gaussian")),
            menu_item("Radial").items([
                menu_item("Spin").on_press(ran("Spin")),
                menu_item("Zoom").on_press(ran("Zoom")),
            ]),
            menu_item("Motion").enabled(false).on_press(ran("Motion")),
        ]),
        menu_item("Noise").items([menu_item("Add noise").on_press(ran("Add noise"))]),
    ]
}

fn row_height(size: ControlSize) -> f32 {
    size.height() * 0.72
}

fn row_center(top: f32, size: ControlSize, rows_above: f32, separators: f32) -> f32 {
    let row = row_height(size);
    top + PADDING + rows_above * row + separators * SEPARATOR + row / 2.0
}

fn context_menu(probe: &Probe) -> impl View {
    let mut stage = column! {
        text("ground")
    }
    .padding(Space::from(EDGE))
    .fill();
    if probe.open {
        stage = column! {
            menu()
                .items(effects())
                .on_dismiss(|probe: &mut Probe| {
                    probe.open = false;
                    probe.dismissals += 1;
                })
        }
        .padding(Space::from(EDGE))
        .fill();
    }
    stage
}

fn context() -> (TypedUi<Probe>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Probe::new(), context_menu, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

fn hover(ui: &mut TypedUi<Probe>, scene: &mut Scene, at: Point) {
    ui.handle_event(Event::PointerMoved(at));
    ui.render(scene);
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

fn press(ui: &mut TypedUi<Probe>, scene: &mut Scene, key: Key) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::NONE,
    });
    ui.render(scene);
}

fn blurs_y() -> f32 {
    row_center(EDGE, ControlSize::Medium, 1.0, 1.0)
}

fn branch_x() -> f32 {
    EDGE + MENU_WIDTH + 40.0
}

#[test]
fn submenu_leaf_runs_and_closes_menu() {
    let (mut ui, mut scene) = context();

    hover(&mut ui, &mut scene, Point::new(EDGE + 60.0, blurs_y()));
    hover(&mut ui, &mut scene, Point::new(branch_x(), blurs_y()));
    click(&mut ui, &mut scene, Point::new(branch_x(), blurs_y()));

    assert_eq!(ui.state().ran, vec!["Gaussian"]);
    assert_eq!(ui.state().dismissals, 1, "menu closed");
}

#[test]
fn keyboard_navigates_submenus() {
    let (mut ui, mut scene) = context();
    hover(&mut ui, &mut scene, Point::new(EDGE + 100.0, EDGE + 2.0));

    press(&mut ui, &mut scene, Key::ArrowDown);
    press(&mut ui, &mut scene, Key::ArrowDown);
    press(&mut ui, &mut scene, Key::ArrowRight);
    press(&mut ui, &mut scene, Key::ArrowDown);
    press(&mut ui, &mut scene, Key::Enter);
    press(&mut ui, &mut scene, Key::ArrowLeft);
    press(&mut ui, &mut scene, Key::Escape);
    assert_eq!(
        ui.state().dismissals,
        0,
        "escape closed root before submenu"
    );
    assert!(ui.state().ran.is_empty());

    press(&mut ui, &mut scene, Key::ArrowDown);
    press(&mut ui, &mut scene, Key::Enter);
    press(&mut ui, &mut scene, Key::Enter);
    assert_eq!(ui.state().ran, vec!["Add noise"]);
    assert_eq!(ui.state().dismissals, 1);
}
