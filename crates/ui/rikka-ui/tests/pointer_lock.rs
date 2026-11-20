use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{CursorShape, Modifiers, Point, PointerButton, Size};
use rikka_ui::{
    Element, Event, EventCtx, Layout, Length, PaintCtx, Rendering, State, TypedUi, Widget,
};

const VIEW: Size = Size::new(200.0, 100.0);

struct Knob;

impl Widget for Knob {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(VIEW.width), Length::px(VIEW.height))
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed { .. } => ctx.lock_pointer(),
            Event::PointerTravelled(by) => {
                ctx.run(move |dial: &mut Dial| dial.turned += by.y);
            }
            Event::PointerReleased { .. } => ctx.release_pointer(),
            _ => {}
        }
    }

    fn cursor(
        &self,
        _state: &Self::State,
        _bounds: rikka_types::Rect,
        _point: Point,
    ) -> Option<CursorShape> {
        Some(CursorShape::Grab)
    }
}

#[derive(Default, State)]
struct Dial {
    turned: f32,
}

fn view(_dial: &Dial) -> Element {
    Element::new(Knob)
}

fn button(pressed: bool) -> Event {
    let position = Point::new(50.0, 50.0);
    let (button, modifiers) = (PointerButton::Primary, Modifiers::NONE);
    match pressed {
        true => Event::PointerPressed {
            position,
            button,
            modifiers,
        },
        false => Event::PointerReleased {
            position,
            button,
            modifiers,
        },
    }
}

fn turned(ui: &mut TypedUi<Dial>) -> f32 {
    let mut turned = 0.0;
    ui.update(|dial| turned = dial.turned);
    turned
}

#[test]
fn locked_pointer_reports_relative_motion() {
    let mut ui = TypedUi::new(Dial::default(), view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(Rendering::new(&mut Scene::new()).with_time(Instant::now()));
    ui.handle_event(Event::PointerMoved(Point::new(50.0, 50.0)));
    assert_eq!(ui.cursor(), CursorShape::Grab);

    ui.handle_event(Event::PointerTravelled(Point::new(0.0, 7.0)));
    assert_eq!(
        turned(&mut ui),
        0.0,
        "relative motion delivered while unlocked"
    );

    ui.handle_event(button(true));
    assert!(ui.pointer_locked());
    assert_eq!(ui.cursor(), CursorShape::Hidden);
    ui.handle_event(Event::PointerTravelled(Point::new(0.0, -3.0)));
    ui.handle_event(Event::PointerTravelled(Point::new(0.0, -2.0)));
    assert_eq!(turned(&mut ui), -5.0);

    ui.handle_event(button(false));
    assert!(!ui.pointer_locked());
    assert_eq!(
        ui.cursor(),
        CursorShape::Grab,
        "pointer not restored after unlock"
    );
}
