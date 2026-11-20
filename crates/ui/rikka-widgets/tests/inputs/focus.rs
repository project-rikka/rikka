use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(400.0, 400.0);
const FRAME: Duration = Duration::from_millis(16);
const FRAMES: usize = 8;
const FIELD: Point = Point::new(60.0, 20.0);
const LABEL: Point = Point::new(60.0, 90.0);
const FOCUSLESS: Point = Point::new(60.0, 160.0);
const CARD: Point = Point::new(300.0, 160.0);
const ROW: Point = Point::new(60.0, 200.0);

#[derive(State)]
struct Form {
    name: TextDocument,
}

fn form_view(form: &Form) -> Element {
    column! {
        text_field(&form.name).fill_width().height(40.0);
        container(text("Plain label")).fill_width().height(Length::px(100.0));
        container(
            container(text("Focusless"))
                .width(Length::px(200.0))
                .on_press(|_: &mut Form| {})
                .focusless(),
        )
        .on_press(|_: &mut Form| {})
        .fill_width()
        .height(Length::px(40.0));
        lazy_column(20, |slot| {
            container(text(format!("row {}", slot.index)))
                .fill_width()
                .height(Length::px(24.0))
        })
        .uniform(24.0)
        .focusable(true)
        .grow(1.0)
        .fill_width();
    }
    .fill()
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
            name: TextDocument::new(""),
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

    fn drag(&mut self, from: Point, to: Point) {
        self.ui.handle_event(Event::PointerMoved(from));
        self.ui.handle_event(Event::PointerPressed {
            position: from,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        });
        self.ui.handle_event(Event::PointerMoved(to));
        self.ui.handle_event(Event::PointerReleased {
            position: to,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        });
        self.settle();
    }

    fn write(&mut self, text: &str) {
        self.ui.handle_event(Event::Commit(text.to_owned()));
        self.settle();
    }
}

#[test]
fn press_moves_focus_toward_focusable_or_clears_it() {
    let cases = [
        ("release outside the field", FIELD, ROW, "x"),
        ("press on plain content", LABEL, LABEL, ""),
        ("press on a focusless control", FOCUSLESS, FOCUSLESS, "x"),
        ("press inside a focusable card", CARD, CARD, ""),
        ("press inside a focusable list", ROW, ROW, ""),
    ];
    for (name, from, to, typed) in cases {
        let mut stage = Stage::new();
        stage.drag(FIELD, FIELD);
        stage.drag(from, to);
        stage.write("x");
        assert_eq!(stage.ui.state().name.text(), typed, "{name}");
    }
}
