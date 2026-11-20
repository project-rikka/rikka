use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size, Stylus, StylusSample, Tool, ToolKind};
use rikka_ui::{
    Element, Event, EventCtx, Layout, Length, PaintCtx, Rendering, State, TypedUi, Widget,
};

const VIEW: Size = Size::new(400.0, 200.0);

#[derive(Clone, Debug, PartialEq)]
struct Heard {
    event: Event,
    stylus: Option<Stylus>,
}

struct Canvas;

impl Widget for Canvas {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(100.0), Length::px(100.0))
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerPressed { .. } => ctx.capture_pointer(),
            Event::PointerReleased { .. } => ctx.release_pointer(),
            Event::PointerMoved(_) => {}
            _ => return,
        }
        let heard = Heard {
            event: event.clone(),
            stylus: ctx.stylus().cloned(),
        };
        ctx.run(move |log: &mut Log| log.heard.push(heard.clone()));
    }
}

#[derive(Default, State)]
struct Log {
    heard: Vec<Heard>,
}

fn view(_log: &Log) -> Element {
    Element::new(Canvas)
}

fn harness() -> TypedUi<Log> {
    let mut ui = TypedUi::new(Log::default(), view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(Rendering::new(&mut Scene::new()).with_time(Instant::now()));
    ui
}

fn heard(ui: &mut TypedUi<Log>) -> Vec<Heard> {
    let mut heard = Vec::new();
    ui.update(|log| heard = std::mem::take(&mut log.heard));
    heard
}

fn sample(x: f32, y: f32, pressure: f32, millis: u64) -> StylusSample {
    StylusSample {
        pressure: Some(pressure),
        contact: true,
        time: Duration::from_millis(millis),
        ..StylusSample::new(Point::new(x, y))
    }
}

fn pen(samples: &[StylusSample]) -> Stylus {
    let mut stylus = Stylus::new(Tool::new(ToolKind::Pen), samples[0]);
    for sample in &samples[1..] {
        stylus.push(*sample);
    }
    stylus
}

fn button(pressed: bool, at: Point) -> Event {
    let (button, modifiers) = (PointerButton::Primary, Modifiers::NONE);
    match pressed {
        true => Event::PointerPressed {
            position: at,
            button,
            modifiers,
        },
        false => Event::PointerReleased {
            position: at,
            button,
            modifiers,
        },
    }
}

#[test]
fn captured_pen_drag_delivers_every_coalesced_sample_to_the_captor() {
    let mut ui = harness();
    let press = pen(&[sample(50.0, 50.0, 0.2, 1)]);
    ui.handle_input(button(true, Point::new(50.0, 50.0)), Some(press.clone()));
    let drag = pen(&[
        sample(120.0, 60.0, 0.4, 2),
        sample(180.0, 70.0, 0.6, 3),
        sample(250.0, 80.0, 0.8, 4),
    ]);
    ui.handle_input(
        Event::PointerMoved(Point::new(250.0, 80.0)),
        Some(drag.clone()),
    );
    let release = pen(&[sample(250.0, 80.0, 0.0, 5)]);
    ui.handle_input(
        button(false, Point::new(250.0, 80.0)),
        Some(release.clone()),
    );
    ui.handle_event(Event::PointerMoved(Point::new(300.0, 90.0)));

    assert_eq!(
        heard(&mut ui),
        vec![
            Heard {
                event: button(true, Point::new(50.0, 50.0)),
                stylus: Some(press),
            },
            Heard {
                event: Event::PointerMoved(Point::new(250.0, 80.0)),
                stylus: Some(drag),
            },
            Heard {
                event: button(false, Point::new(250.0, 80.0)),
                stylus: Some(release),
            },
        ],
        "the drag outside the canvas reaches it only while captured"
    );
}

#[test]
fn mouse_input_reports_no_stylus() {
    let mut ui = harness();
    ui.handle_input(
        Event::PointerMoved(Point::new(20.0, 20.0)),
        Some(pen(&[sample(20.0, 20.0, 1.0, 1)])),
    );
    ui.handle_event(Event::PointerMoved(Point::new(30.0, 30.0)));
    ui.handle_event(button(true, Point::new(30.0, 30.0)));

    let heard = heard(&mut ui);
    assert_eq!(heard.len(), 3);
    assert_eq!(
        heard[0].stylus.as_ref().and_then(Stylus::pressure),
        Some(1.0)
    );
    assert!(heard[1..].iter().all(|heard| heard.stylus.is_none()));
}
