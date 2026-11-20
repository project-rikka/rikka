use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::*;

use crate::common::{self, UNCOVERED, renderer};

const VIEW: u32 = 128;
const LIT: f32 = 64.0;
const EXACT: i32 = 2;

#[derive(State)]
struct Modal {
    open: bool,
}

fn modal_view(modal: &Modal) -> Element {
    let lit = container(column_of(Vec::<Element>::new()))
        .background(Color::WHITE)
        .width(Length::px(LIT))
        .height(Length::FILL)
        .build();
    let blurred = modal.open.then(|| {
        scrim()
            .on_dismiss(|modal: &mut Modal| modal.open = false)
            .build()
    });
    column_of(std::iter::once(lit).chain(blurred))
        .fill()
        .build()
}

fn modal() -> TypedUi<Modal> {
    let mut ui = TypedUi::new(Modal { open: true }, modal_view, Theme::dark());
    ui.set_background(Some(UNCOVERED));
    ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
    ui
}

fn click(ui: &mut TypedUi<Modal>, at: Point) {
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
}

fn near(shown: (u8, u8, u8, u8), expected: (i32, i32, i32)) -> bool {
    let (r, g, b, _) = shown;
    [(r, expected.0), (g, expected.1), (b, expected.2)]
        .into_iter()
        .all(|(channel, wanted)| (i32::from(channel) - wanted).abs() <= EXACT)
}

#[test]
fn scrim_blurs_without_shading_under_pointer() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();
    let mut ui = modal();
    let start = Instant::now();
    let rests = [
        None,
        Some(Point::new(8.0, 8.0)),
        Some(Point::new(96.0, 64.0)),
    ];

    for (step, rest) in rests.into_iter().enumerate() {
        if let Some(at) = rest {
            ui.handle_event(Event::PointerMoved(at));
        }
        let settled = start + Duration::from_secs(2 * (step as u64 + 1));
        ui.render(Rendering::new(&mut scene).with_time(settled));
        renderer.draw(&mut scene, &mut target).unwrap();
        let pixels = renderer.read_pixels(&mut target).unwrap();
        let at = |x: usize| common::pixel(&pixels, VIEW as usize, x, 64);
        assert!(
            near(at(2), (255, 255, 255)),
            "pointer at {rest:?}: lit side shows {:?}",
            at(2)
        );
        assert!(
            near(at(125), (255, 0, 0)),
            "pointer at {rest:?}: red side shows {:?}",
            at(125)
        );
        let (_, green, _, _) = at(LIT as usize);
        assert!(
            (64..=192).contains(&green),
            "pointer at {rest:?}: the edge stays sharp at {:?}",
            at(LIT as usize)
        );
    }
}

#[test]
fn scrim_press_dismisses() {
    let mut scene = Scene::new();
    let mut ui = modal();
    ui.render(&mut scene);

    click(&mut ui, Point::new(64.0, 64.0));
    ui.render(&mut scene);

    assert!(!ui.state().open);
}
