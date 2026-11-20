use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::{Rendering, State, TypedUi, View, skip_view, text};

#[derive(Default, State)]
struct Counter {
    count: i32,
}

impl Counter {
    fn set(&mut self, value: i32) {
        self.count = value;
    }

    fn same(&mut self, value: i32) {
        if value == self.count {
            skip_view();
            return;
        }
        self.count = value;
    }
}

fn counted(counter: &Counter) -> impl View {
    text(format!("{}", counter.count))
}

struct Harness {
    ui: TypedUi<Counter>,
    scene: Scene,
    now: Instant,
}

impl Harness {
    fn new() -> Self {
        let mut harness = Self {
            ui: TypedUi::new(Counter::default(), counted, Theme::dark()),
            scene: Scene::new(),
            now: Instant::now(),
        };
        harness.ui.set_viewport(Size::new(200.0, 100.0), 1.0);
        harness.frame();
        harness
    }

    fn frame(&mut self) {
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.now));
    }

    fn needs_frame(&self) -> bool {
        self.ui.needs_render()
    }
}

#[test]
fn noop_update_requests_no_frame() {
    let mut harness = Harness::new();
    assert!(!harness.needs_frame(), "runtime idle after frame");

    harness.ui.update(|counter| counter.same(0));

    assert!(!harness.needs_frame());
}

#[test]
fn skip_view_keeps_earlier_change() {
    let mut harness = Harness::new();

    harness.ui.update(|counter| counter.set(1));
    harness.ui.update(|counter| counter.same(1));

    assert!(harness.needs_frame(), "earlier change lost its rebuild");
}
