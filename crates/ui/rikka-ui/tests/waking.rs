use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::{PaintCtx, Rendering, State, TypedUi, View, Widget};

const TURN: Duration = Duration::from_secs(30);

#[derive(Widget)]
struct Blinker {
    painted: Rc<Cell<u32>>,
}

#[derive(Default)]
struct BlinkerState {
    since: Option<Instant>,
}

impl Widget for Blinker {
    type State = BlinkerState;

    fn paint(&self, state: &mut BlinkerState, ctx: &mut PaintCtx<'_>) {
        self.painted.set(self.painted.get() + 1);
        let since = *state.since.get_or_insert(ctx.now());
        let turns = ctx.now().saturating_duration_since(since).as_secs() / TURN.as_secs() + 1;
        ctx.wake_at(since + TURN * u32::try_from(turns).expect("several frames"));
    }
}

#[derive(Default, State)]
struct Watched {
    painted: Rc<Cell<u32>>,
}

fn watched(watched: &Watched) -> impl View {
    Blinker {
        painted: Rc::clone(&watched.painted),
    }
}

fn open_ui(now: Instant) -> (TypedUi<Watched>, Scene) {
    let mut ui = TypedUi::new(Watched::default(), watched, Theme::dark());
    let mut scene = Scene::new();
    ui.set_viewport(Size::new(200.0, 100.0), 1.0);
    ui.render(Rendering::new(&mut scene).with_time(now));
    (ui, scene)
}

#[test]
fn wake_deadline_redraws_on_time() {
    let now = Instant::now();
    let (mut ui, mut scene) = open_ui(now);
    let painted = Rc::clone(&ui.state().painted);
    assert_eq!(painted.get(), 1);
    assert_eq!(ui.next_wake(), Some(now + TURN));
    assert!(!ui.needs_render(), "no frame before next wake");

    ui.render(Rendering::new(&mut scene).with_time(now + TURN / 2));
    assert_eq!(painted.get(), 1);

    ui.render(Rendering::new(&mut scene).with_time(now + TURN));
    assert_eq!(painted.get(), 2);
    assert_eq!(ui.next_wake(), Some(now + TURN * 2));
}
