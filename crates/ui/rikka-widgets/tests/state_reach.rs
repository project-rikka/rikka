use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::column;

const VIEW: Size = Size::new(200.0, 200.0);
const TAP: f32 = 50.0;

struct Tap {
    press: Handler,
}

fn tap<Marker>(press: impl IntoHandler<(), (), Marker>) -> Element {
    Element::new(Tap {
        press: press.into_handler(),
    })
}

impl Widget for Tap {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(TAP))
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed { .. } = event {
            ctx.call(&self.press, ());
            ctx.set_handled();
        }
    }
}

fn open_ui<S: State>(state: S, view: impl for<'a> ViewFn<'a, S>) -> (TypedUi<S>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(state, view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

fn tapped<S: State>(ui: &mut TypedUi<S>, scene: &mut Scene, slot: usize) {
    let at = Point::new(VIEW.width / 2.0, TAP * slot as f32 + TAP / 2.0);
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

#[derive(Default, State)]
struct Keyboard {
    pressed: u32,
}

#[derive(Default, State)]
struct Calculator {
    #[state]
    keyboard: Keyboard,
    #[state]
    memory: Keyboard,
}

fn calculator_view(_calculator: &Calculator) -> Element {
    column! {
        scope(
            |calculator: &mut Calculator| &mut calculator.keyboard,
            tap(|keyboard: &mut Keyboard| keyboard.pressed += 1),
        );
        scope(
            |calculator: &mut Calculator| &mut calculator.memory,
            tap(|keyboard: &mut Keyboard| keyboard.pressed += 10),
        );
    }
    .fill_width()
    .build()
}

#[test]
fn scope_selects_handler_target_state() {
    let (mut ui, mut scene) = open_ui(Calculator::default(), calculator_view);

    tapped(&mut ui, &mut scene, 1);
    assert_eq!(ui.state().keyboard.pressed, 0);
    assert_eq!(ui.state().memory.pressed, 10);

    tapped(&mut ui, &mut scene, 0);
    assert_eq!(ui.state().keyboard.pressed, 1);
    assert_eq!(ui.state().memory.pressed, 10);
}
