use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(200.0, 120.0);

const LAID: Size = Size::new(40.0, 20.0);

const FACTOR: f32 = 2.0;

#[derive(Default, State)]
struct Presses {
    count: u32,
}

fn magnified(_presses: &Presses) -> impl View {
    column_of([scaled(
        button("Go")
            .on_press(|presses: &mut Presses| presses.count += 1)
            .build()
            .width(LAID.width)
            .height(LAID.height),
        FACTOR,
    )])
    .build()
    .fill()
}

fn click(ui: &mut TypedUi<Presses>, scene: &mut Scene, at: Point) -> bool {
    ui.handle_event(Event::PointerMoved(at));
    let pressed = ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut *scene);
    pressed
}

#[test]
fn scaled_controls_are_hit_where_they_are_drawn() {
    let cases = [
        (
            Point::new(LAID.width * 1.5, LAID.height * 1.5),
            1,
            "beyond the laid box, within the drawn one",
        ),
        (
            Point::new(LAID.width * FACTOR + 10.0, LAID.height),
            0,
            "beyond the drawn box",
        ),
    ];
    for (at, expected, case) in cases {
        let mut scene = Scene::new();
        let mut ui = TypedUi::new(Presses::default(), magnified, Theme::dark());
        ui.set_viewport(VIEW, 1.0);
        ui.render(&mut scene);
        ui.render(&mut scene);
        let pressed = click(&mut ui, &mut scene, at);
        assert_eq!(ui.state().count, expected, "{case}");
        assert_eq!(
            pressed,
            expected == 1,
            "{case}: the press reports whether it was used"
        );
    }
}
