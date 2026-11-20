use std::time::Instant;

use rikka_nested::{Input, View, nested_view};
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::{
    Behavior, Element, Event, Layout, Length, PaintCtx, Rendering, State, Styled, TypedUi, Widget,
};

use crate::support::{self, Client, roundtrip_until};

const VIEW: Size = Size::new(200.0, 80.0);
const PAGE: Point = Point::new(20.0, 20.0);
const ASIDE: Point = Point::new(160.0, 20.0);
const MARK: Point = Point::new(7.0, 9.0);

struct Row;

impl Widget for Row {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}
}

struct Aside;

impl Widget for Aside {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(60.0), Length::FILL)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::FOCUSABLE
    }
}

#[derive(State)]
struct Desk {
    view: View,
    retained: bool,
}

fn desk(desk: &Desk) -> Element {
    let page = nested_view(&desk.view)
        .retain_focus(desk.retained)
        .width(Length::px(140.0))
        .height(Length::FILL);
    Element::with_children(Row, vec![Element::new(page), Element::new(Aside)])
}

fn render(ui: &mut TypedUi<Desk>) {
    ui.render(Rendering::new(&mut Scene::new()).with_time(Instant::now()));
}

fn click(ui: &mut TypedUi<Desk>, at: Point) {
    for pressed in [true, false] {
        let (button, modifiers, position) = (PointerButton::Primary, Modifiers::NONE, at);
        ui.handle_event(match pressed {
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
        });
    }
    render(ui);
}

fn kept_until_marked(client: &mut Client, view: &View, surface: u32) -> bool {
    view.deliver(Input::PointerMoved { at: MARK });
    let marked = format!("motion {} {}", MARK.x, MARK.y);
    roundtrip_until(client, |client| client.received.pointer.contains(&marked));
    !client
        .received
        .keyboard
        .contains(&format!("leave {surface}"))
}

#[test]
fn retained_focus_keeps_the_client_focused_until_released() {
    for retained in [true, false] {
        let (nested, _events) = support::start();
        let app_id = format!("rikka-view-retain-{retained}");
        let view = nested.view(&app_id);
        let mut client = Client::connect(nested.socket());
        client.follow(&view);
        let _pointer = client.pointer();
        let _keyboard = client.keyboard();
        let window = client.window(Some(&app_id));
        client.map(&window, 140, 80);
        roundtrip_until(&mut client, |_| view.look().attached);
        let surface = window.surface.id();

        let mut ui = TypedUi::new(
            Desk {
                view: view.clone(),
                retained,
            },
            desk,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        render(&mut ui);
        click(&mut ui, PAGE);
        roundtrip_until(&mut client, |client| {
            client
                .received
                .keyboard
                .contains(&format!("enter {surface}"))
        });

        click(&mut ui, ASIDE);
        assert_eq!(
            kept_until_marked(&mut client, &view, surface),
            retained,
            "retained: {retained}"
        );

        ui.update(|desk| desk.retained = false);
        render(&mut ui);
        roundtrip_until(&mut client, |client| {
            client
                .received
                .keyboard
                .contains(&format!("leave {surface}"))
        });
    }
}
