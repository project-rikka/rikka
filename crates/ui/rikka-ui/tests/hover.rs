use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{
    Element, Event, EventCtx, Layout, Length, PaintCtx, Rendering, State, TypedUi, Widget,
};

const VIEW: Size = Size::new(400.0, 200.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hover {
    name: &'static str,
    entered: bool,
}

struct Tracker {
    name: &'static str,
    frame: Rect,
}

impl Widget for Tracker {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(
                Length::px(self.frame.size.width),
                Length::px(self.frame.size.height),
            )
            .placed_at(self.frame.origin)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let entered = match event {
            Event::PointerEntered => true,
            Event::PointerExited => false,
            _ => return,
        };
        let hover = Hover {
            name: self.name,
            entered,
        };
        ctx.run(move |probe: &mut Probe| probe.log.push(hover));
    }
}

fn tracker(name: &'static str, frame: Rect, children: Vec<Element>) -> Element {
    Element::with_children(Tracker { name, frame }, children)
}

#[derive(Default, State)]
struct Probe {
    log: Vec<Hover>,
    swapped: bool,
}

fn view(probe: &Probe) -> Element {
    let left = tracker(
        "left",
        Rect::new(0.0, 0.0, 200.0, 200.0),
        vec![tracker(
            "inner",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Vec::new(),
        )],
    )
    .keyed("left");
    let right = tracker("right", Rect::new(200.0, 0.0, 200.0, 200.0), Vec::new()).keyed("right");
    let halves = if probe.swapped {
        vec![right, left]
    } else {
        vec![left, right]
    };
    tracker("root", Rect::new(0.0, 0.0, VIEW.width, VIEW.height), halves)
}

struct Harness {
    ui: TypedUi<Probe>,
    scene: Scene,
    start: Instant,
}

impl Harness {
    fn new() -> Self {
        let mut harness = Self {
            ui: TypedUi::new(Probe::default(), view, Theme::dark()),
            scene: Scene::new(),
            start: Instant::now(),
        };
        harness.ui.set_viewport(VIEW, 1.0);
        harness.frame();
        harness
    }

    fn frame(&mut self) {
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.start));
    }

    fn take_events(&mut self) -> Vec<Hover> {
        let mut events = Vec::new();
        self.ui
            .update(|probe| events = std::mem::take(&mut probe.log));
        events
    }

    fn moved(&mut self, x: f32, y: f32) -> Vec<Hover> {
        self.ui.handle_event(Event::PointerMoved(Point::new(x, y)));
        self.take_events()
    }

    fn left(&mut self) -> Vec<Hover> {
        self.ui.handle_event(Event::PointerLeft);
        self.take_events()
    }
}
fn entered(name: &'static str) -> Hover {
    Hover {
        name,
        entered: true,
    }
}

fn exited(name: &'static str) -> Hover {
    Hover {
        name,
        entered: false,
    }
}

#[test]
fn hover_exits_before_enters_and_skips_shared() {
    let mut harness = Harness::new();
    assert_eq!(
        harness.moved(250.0, 100.0),
        vec![entered("right"), entered("root")]
    );
    assert!(harness.moved(260.0, 110.0).is_empty());
    assert_eq!(
        harness.moved(150.0, 150.0),
        vec![exited("right"), entered("left")]
    );
    assert_eq!(harness.moved(50.0, 50.0), vec![entered("inner")]);
    assert_eq!(
        harness.left(),
        vec![exited("inner"), exited("left"), exited("root")]
    );
}
#[test]
fn relayout_keeps_hover_under_cursor() {
    let mut harness = Harness::new();
    harness.moved(50.0, 50.0);
    harness.ui.update(|probe| probe.swapped = true);
    harness.frame();
    assert!(harness.take_events().is_empty());
    assert_eq!(
        harness.moved(250.0, 100.0),
        vec![exited("inner"), exited("left"), entered("right")]
    );
}
