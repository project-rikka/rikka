use std::time::{Duration, Instant};

use rikka_renderer::{Scene, SceneFrame};
use rikka_theme::Theme;
use rikka_types::{Color, Rect, Size, WindowView};
use rikka_ui::{Element, Layout, Length, PaintCtx, Rendering, State, TypedUi, View, Widget};

const VIEW: Size = Size::new(400.0, 300.0);

struct Portal {
    x: f32,
}

impl Widget for Portal {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        ctx.draw_window(WindowView::new(
            "window",
            Rect::new(self.x, 40.0, 120.0, 80.0),
        ));
    }

    fn same(&self, other: &Self) -> bool {
        self.x == other.x
    }
}

#[derive(State)]
struct Held {
    x: f32,
}

fn held(held: &Held) -> impl View {
    Element::new(Portal { x: held.x })
}

#[test]
fn moved_window_view_alone_damages_scene() {
    let mut ui = TypedUi::new(Held { x: 20.0 }, held, Theme::dark());
    ui.set_background(Some(Color::TRANSPARENT));
    ui.set_viewport(VIEW, 2.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    scene.take_frame(&mut SceneFrame::default());

    ui.update(|held| held.x = 200.0);
    ui.render(Rendering::new(&mut scene).with_time(now + Duration::from_millis(16)));

    assert_eq!(ui.windows()[0].rect, Rect::new(200.0, 40.0, 120.0, 80.0));
    assert!(
        scene.has_changes(),
        "frame moving only a window view would never be presented"
    );
}
