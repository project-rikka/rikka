use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Point, Size};
use rikka_ui::{
    Anchor, Element, Event, EventCtx, Layout, Length, Rendering, State, TypedUi, Widget,
};

const VIEW: Size = Size::new(400.0, 200.0);
const STAGE: &str = "stage";

struct Tracker {
    name: &'static str,
    size: Size,
}

impl Widget for Tracker {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::px(self.size.width), Length::px(self.size.height))
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerEntered = event {
            let name = self.name;
            ctx.run(move |probe: &mut Probe| probe.entered.push(name));
        }
    }
}

fn tracker(name: &'static str, width: f32, height: f32) -> Element {
    Element::new(Tracker {
        name,
        size: Size::new(width, height),
    })
}

#[derive(Default, State)]
struct Probe {
    entered: Vec<&'static str>,
}

fn view(_probe: &Probe) -> Element {
    Element::with_children(
        Tracker {
            name: "root",
            size: VIEW,
        },
        vec![
            tracker("stage", VIEW.width, VIEW.height).keyed(STAGE),
            tracker("chip", 40.0, 40.0).anchored(Anchor::centered_on(STAGE)),
            tracker("cover", 10.0, 10.0).anchored(Anchor::covering(STAGE)),
        ],
    )
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
        for _ in 0..3 {
            harness
                .ui
                .render(Rendering::new(&mut harness.scene).with_time(harness.start));
        }
        harness
    }

    fn entered_at(&mut self, x: f32, y: f32) -> Vec<&'static str> {
        self.ui.handle_event(Event::PointerMoved(Point::new(x, y)));
        let mut entered = Vec::new();
        self.ui
            .update(|probe| entered = std::mem::take(&mut probe.entered));
        entered
    }
}

#[test]
fn covering_anchor_spans_target_on_top() {
    let mut harness = Harness::new();
    assert!(harness.entered_at(1.0, 1.0).contains(&"cover"));
    assert_eq!(
        harness.entered_at(VIEW.width - 1.0, VIEW.height - 1.0),
        Vec::<&str>::new()
    );
    assert_eq!(
        harness.entered_at(VIEW.width / 2.0, VIEW.height / 2.0),
        Vec::<&str>::new()
    );
}
