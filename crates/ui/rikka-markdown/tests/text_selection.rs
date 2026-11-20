use rikka_sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rikka_markdown::{markdown, markdown_text};
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;

const VIEW: Size = Size::new(420.0, 420.0);
const SETTLED: Duration = Duration::from_secs(5);

const SAID: &str = "first paragraph\n\nsecond paragraph";

const DOCUMENT: &str = "\
# Title

Some prose with ![a picture](https://example.test/one.png) in it.

- one
- two

| head | other |
|------|-------|
| a    |       |

```rust
fn main() {}
```
";

fn board() -> MutexGuard<'static, ()> {
    static ORDER: Mutex<()> = Mutex::new(());
    let order = ORDER.lock();
    rikka_ui::clipboard::set_text("what was there before");
    order
}

fn copied() -> String {
    rikka_ui::clipboard::text().unwrap_or_default()
}

#[derive(State)]
struct Reader {
    source: String,
}

fn reading(reader: &Reader) -> Element {
    markdown(reader.source.as_str())
        .fill_width()
        .selectable("document")
        .build()
}

fn read(source: &str) -> TypedUi<Reader> {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Reader {
            source: source.to_owned(),
        },
        reading,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    ui.render(Rendering::new(&mut scene).with_time(now + SETTLED));
    ui
}

fn pointer(ui: &mut TypedUi<Reader>, event: fn(Point) -> Event, at: Point) {
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(event(at));
}

fn pressed(position: Point) -> Event {
    Event::PointerPressed {
        position,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    }
}

fn released(position: Point) -> Event {
    Event::PointerReleased {
        position,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    }
}

fn typed(ui: &mut TypedUi<Reader>, character: char) {
    ui.handle_event(Event::KeyPressed {
        key: Key::Char(character),
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::CONTROL,
    });
}

fn copied_whole(source: &str, at: Point) -> String {
    let mut ui = read(source);
    pointer(&mut ui, pressed, at);
    pointer(&mut ui, released, at);
    typed(&mut ui, 'a');
    typed(&mut ui, 'c');
    copied()
}

#[test]
fn drag_selection_copies_spanned_runs() {
    let _order = board();
    let mut ui = read(SAID);

    pointer(&mut ui, pressed, Point::new(1.0, 6.0));
    pointer(
        &mut ui,
        released,
        Point::new(VIEW.width - 1.0, VIEW.height - 1.0),
    );
    typed(&mut ui, 'c');

    assert_eq!(copied(), SAID);
}

#[test]
fn select_all_copies_reading_order() {
    let _order = board();

    assert_eq!(
        copied_whole(DOCUMENT, Point::new(1.0, 6.0)),
        "Title\n\nSome prose with a picture in it.\n\none\ntwo\n\n\
         head\tother\na\t\n\nfn main() {}"
    );
}

#[test]
fn streamed_document_copies_whole() {
    let _order = board();
    let prose: String = (0..400)
        .map(|at| format!("Paragraph {at} of a document too long to build at once.\n\n"))
        .collect();
    let lines: String = (0..500)
        .map(|at| format!("let line{at} = {at};\n"))
        .collect();
    let listing = format!("```rust\n{lines}```\n");

    for (source, at, foot) in [
        (prose, Point::new(1.0, 6.0), "Paragraph 399"),
        (listing, Point::new(30.0, 44.0), "let line499 = 499;"),
    ] {
        let whole = copied_whole(&source, at);
        assert_eq!(whole, markdown_text(&source).as_ref());
        assert!(whole.contains(foot), "unreached bottom built");
    }
}

#[test]
fn code_copy_button_copies_block_alone() {
    let _order = board();
    let mut ui = read("```bash\nnix develop\nbazel build //...\n```\n\nAfter the block.");
    let at = Point::new(VIEW.width - 24.0, 18.0);

    pointer(&mut ui, pressed, at);
    pointer(&mut ui, released, at);

    assert_eq!(copied(), "nix develop\nbazel build //...");
}
