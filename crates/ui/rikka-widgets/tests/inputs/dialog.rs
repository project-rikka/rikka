use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(480.0, 320.0);
const FRAME: Duration = Duration::from_millis(16);
const FRAMES: usize = 30;

#[derive(State)]
struct Form {
    ready: bool,
    accepted: usize,
    cancelled: usize,
}

fn form_view(form: &Form) -> Element {
    dialog("Rename", text("Body"))
        .cancel("Cancel", |form: &mut Form| form.cancelled += 1)
        .accept("Rename", |form: &mut Form| form.accepted += 1)
        .ready(form.ready)
        .build()
}

struct Stage {
    ui: TypedUi<Form>,
    scene: Scene,
    now: Instant,
}

impl Stage {
    fn new() -> Stage {
        let form = Form {
            ready: false,
            accepted: 0,
            cancelled: 0,
        };
        let mut ui = TypedUi::new(form, form_view, Theme::dark());
        ui.set_viewport(VIEW, 1.0);
        let mut stage = Stage {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
        };
        stage.settle();
        stage
    }

    fn settle(&mut self) {
        for _ in 0..FRAMES {
            self.now += FRAME;
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.now));
        }
    }

    fn press(&mut self, key: Key) {
        self.ui.handle_event(Event::KeyPressed {
            key,
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers: Modifiers::NONE,
        });
        self.settle();
    }
}

#[test]
fn dialog_accepts_on_enter_only_when_ready_and_cancels_on_escape() {
    let mut stage = Stage::new();

    stage.press(Key::Enter);
    assert_eq!(stage.ui.state().accepted, 0);

    stage.ui.update(|form| form.ready = true);
    stage.settle();
    stage.press(Key::Enter);
    assert_eq!(stage.ui.state().accepted, 1);

    stage.press(Key::Escape);
    assert_eq!(stage.ui.state().cancelled, 1);
    assert_eq!(stage.ui.state().accepted, 1);
}
