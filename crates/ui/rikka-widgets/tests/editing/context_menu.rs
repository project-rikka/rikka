use std::sync::Once;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Rect, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(640.0, 480.0);
const WORDS: &str = "hello world";
const PROSE: &str = "words to read and copy";
const SETTLE: Duration = Duration::from_secs(1);
const SETTLE_FRAMES: usize = 8;

#[derive(State)]
struct Form {
    field: TextDocument,
}

fn form(form: &Form) -> impl View {
    column! {
        text_field(&form.field).width(Length::px(300.0));
        rich_text(PROSE).selectable("prose");
    }
    .gap(Space::WIDE)
    .padding(Space::WIDE)
    .fill()
}

struct Harness {
    ui: TypedUi<Form>,
    scene: Scene,
    now: Instant,
}

impl Harness {
    fn new() -> Self {
        static CATALOGS: Once = Once::new();
        CATALOGS.call_once(|| {
            I18n::builder()
                .embedded(rikka_widgets::CATALOGS)
                .locale("en")
                .install()
                .expect("widget catalogs");
        });
        let mut harness = Self {
            ui: TypedUi::new(
                Form {
                    field: TextDocument::new(WORDS),
                },
                form,
                Theme::dark(),
            ),
            scene: Scene::new(),
            now: Instant::now(),
        };
        harness.ui.set_viewport(VIEW, 1.0);
        harness.settle();
        harness
    }

    fn settle(&mut self) {
        for _ in 0..SETTLE_FRAMES {
            self.now += SETTLE;
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.now));
            if !self.ui.needs_render() {
                break;
            }
        }
    }

    fn press(&mut self, at: Point, button: PointerButton) {
        self.ui.handle_event(Event::PointerMoved(at));
        self.ui.handle_event(Event::PointerPressed {
            position: at,
            button,
            modifiers: Modifiers::NONE,
        });
        self.ui.handle_event(Event::PointerReleased {
            position: at,
            button,
            modifiers: Modifiers::NONE,
        });
        self.settle();
    }

    fn widget(&self, name: &str) -> Rect {
        let dump = self.ui.dump();
        dump.lines()
            .find_map(|line| line.trim_start().strip_prefix(&format!("{name} [")))
            .map(rectangle)
            .unwrap_or_else(|| panic!("no {name} in\n{dump}"))
    }

    fn item(&self, label: &str) -> Option<Rect> {
        self.ui
            .dump()
            .lines()
            .filter(|line| line.contains(&format!("\"{label}\"")))
            .find_map(|line| line.trim_start().strip_prefix("· text ["))
            .map(rectangle)
    }

    fn choose(&mut self, label: &str) {
        let row = self
            .item(label)
            .unwrap_or_else(|| panic!("no {label} in the menu\n{}", self.ui.dump()));
        self.press(row.center(), PointerButton::Primary);
    }

    fn field(&self) -> TextDocument {
        self.ui.state().field.clone()
    }
}

fn rectangle(line: &str) -> Rect {
    let (at, _) = line.split_once(']').expect("closed rectangle");
    let numbers: Vec<f32> = at
        .split([' ', 'x'])
        .map(|number| number.parse().expect("number"))
        .collect();
    Rect::new(numbers[0], numbers[1], numbers[2], numbers[3])
}

fn point_in_field(harness: &Harness) -> Point {
    let field = harness.widget("TextField");
    Point::new(field.x() + 24.0, field.center().y)
}

#[test]
fn field_context_menu_runs_command() {
    let mut harness = Harness::new();
    let at = point_in_field(&harness);

    harness.press(at, PointerButton::Secondary);
    for label in [
        "Undo",
        "Redo",
        "Cut",
        "Copy",
        "Paste",
        "Delete",
        "Select All",
    ] {
        assert!(harness.item(label).is_some(), "{label} is missing");
    }
    assert!(harness.item("Ctrl+C").is_some(), "shortcuts missing");

    harness.choose("Select All");
    assert_eq!(harness.field().selection(), 0..WORDS.len());
    assert!(
        harness.item("Undo").is_none(),
        "menu still open after choice"
    );

    harness.press(at, PointerButton::Secondary);
    assert_eq!(
        harness.field().selection(),
        0..WORDS.len(),
        "right click inside selection cleared it"
    );
    harness.choose("Copy");
    assert_eq!(clipboard::text().as_deref(), Some(WORDS));

    harness.press(at, PointerButton::Secondary);
    harness.choose("Delete");
    assert_eq!(harness.field().text(), "");

    harness.press(at, PointerButton::Secondary);
    harness.choose("Undo");
    assert_eq!(harness.field().text(), WORDS);
}
