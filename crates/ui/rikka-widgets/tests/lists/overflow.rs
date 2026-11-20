use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Rect, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(480.0, 300.0);
const SETTLED: Duration = Duration::from_secs(2);

const SHALLOW: f32 = 10.0;
const LEAD: f32 = 40.0;

type Watch = Rc<Cell<Rect>>;

struct Watched {
    at: Watch,
}

impl Widget for Watched {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().width(Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        self.at.set(ctx.bounds());
    }
}

fn watched(at: &Watch, child: impl View) -> Element {
    Element::with_children(Watched { at: at.clone() }, vec![child.build()])
}

fn pressable(at: &Watch) -> Element {
    watched(
        at,
        button(text("press me")).on_press(|page: &mut Page| page.pressed = true),
    )
}

fn clipped(at: &Watch) -> Element {
    scrollable(
        column! {
            spacer().height(Length::px(LEAD)).shrink(0.0);
            pressable(at)
        }
        .fill_width(),
    )
    .height(Length::px(SHALLOW))
    .fill_width()
    .build()
}

type Shape = fn(&Watch) -> Element;

#[derive(State)]
struct Page {
    shape: Shape,
    at: Watch,
    pressed: bool,
}

fn page_view(page: &Page) -> impl View {
    (page.shape)(&page.at)
}

struct Harness {
    ui: TypedUi<Page>,
}

impl Harness {
    fn showing(shape: Shape) -> Self {
        let mut harness = Harness {
            ui: TypedUi::new(
                Page {
                    shape,
                    at: Watch::default(),
                    pressed: false,
                },
                page_view,
                Theme::dark(),
            ),
        };
        let mut scene = Scene::new();
        harness.ui.set_viewport(VIEW, 1.0);
        let start = Instant::now();
        harness
            .ui
            .render(Rendering::new(&mut scene).with_time(start));
        harness
            .ui
            .render(Rendering::new(&mut scene).with_time(start + SETTLED));
        harness
    }

    fn painted(&self) -> Rect {
        self.ui.state().at.get()
    }

    fn press(&mut self, point: Point) -> bool {
        for event in [
            Event::PointerMoved(point),
            Event::PointerPressed {
                position: point,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            Event::PointerReleased {
                position: point,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
        ] {
            self.ui.handle_event(event);
        }
        self.ui.state().pressed
    }
}

#[test]
fn clipped_content_ignores_press() {
    let mut harness = Harness::showing(clipped);
    let button = harness.painted();

    assert!(button.min_y() > SHALLOW, "button above its surface");
    assert!(
        !harness.press(button.center()),
        "press reached clipped content"
    );
}
