use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Rect, RoundedRect, Size};
use rikka_ui::{Element, Layout, Length, PaintCtx, Quad, Rendering, State, TypedUi, View, Widget};

const VIEW: Size = Size::new(400.0, 300.0);

struct Slab;

impl Widget for Slab {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(200.0), Length::px(100.0))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(Color::WHITE).radius(8.0));
        ctx.opaque(RoundedRect::new(bounds, 8.0));
    }
}

struct Fader {
    opacity: f32,
}

impl Widget for Fader {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        ctx.set_subtree_opacity(self.opacity);
    }

    fn same(&self, other: &Self) -> bool {
        self.opacity == other.opacity
    }
}

#[derive(Default, State)]
struct Fading {
    opacity: f32,
}

fn fading(fading: &Fading) -> impl View {
    Element::with_children(
        Fader {
            opacity: fading.opacity,
        },
        vec![Element::new(Slab)],
    )
}

#[test]
fn opaque_region_in_logical_pixels() {
    let mut ui = TypedUi::new(Fading { opacity: 1.0 }, fading, Theme::dark());
    ui.set_background(Some(Color::TRANSPARENT));
    ui.set_viewport(VIEW, 2.0);
    let mut scene = Scene::new();
    ui.render(Rendering::new(&mut scene).with_time(Instant::now()));
    let slab = Rect::new(0.0, 0.0, 200.0, 100.0);
    assert_eq!(
        ui.opaque()
            .iter()
            .map(|shape| shape.rect)
            .collect::<Vec<_>>(),
        vec![slab],
        "opaque slab reported in logical pixels"
    );

    ui.update(|fading| fading.opacity = 0.5);
    ui.render(Rendering::new(&mut scene).with_time(Instant::now()));
    assert!(
        ui.opaque().is_empty(),
        "faded slab reported opaque: {:?}",
        ui.opaque()
    );

    ui.set_background(Some(Color::BLACK));
    ui.update(|fading| fading.opacity = 1.0);
    ui.render(Rendering::new(&mut scene).with_time(Instant::now()));
    assert!(
        ui.opaque()
            .iter()
            .any(|shape| shape.rect == Rect::new(0.0, 0.0, VIEW.width, VIEW.height)),
        "opaque background not reported as whole surface: {:?}",
        ui.opaque()
    );
}
