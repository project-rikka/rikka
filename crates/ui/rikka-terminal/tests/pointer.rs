use std::time::Instant;

use rikka_types::{Key, Modifiers, Point, Size};
use rikka_ui::clipboard::{self, Selection};

use crate::support::{POLL_INTERVAL, Running, TIMEOUT, alone};

#[test]
fn pointer_selection_sets_primary() {
    let _alone = alone();
    clipboard::set_text_of(Selection::Primary, "");
    let mut running = Running::start("printf 'taken by the pointer\\n'; sleep 30");

    running.wait_for("taken by the pointer", Modifiers::NONE);

    let primary = clipboard::text_of(Selection::Primary).unwrap_or_default();
    assert!(
        primary.contains("taken by the pointer"),
        "primary selection: {primary:?}",
    );
}

#[test]
fn mouse_reporting_encodes_position() {
    let _alone = alone();
    let mut running = Running::start("printf '\\033[?1006h\\033[?1000h'; cat -v");

    let deadline = Instant::now() + TIMEOUT;
    let output = loop {
        running.click(Point::new(0.5, 0.5), Modifiers::NONE);
        let output = running.swept(Modifiers::SHIFT);
        if output.contains("^[[<0;1;1M") {
            break output;
        }
        assert!(
            Instant::now() < deadline,
            "no mouse report received, saw {output:?}",
        );
        std::thread::sleep(POLL_INTERVAL);
    };

    assert!(
        output.contains("^[[<0;1;1m"),
        "no release report: {output:?}",
    );
}

#[test]
fn context_menu_select_all() {
    let _alone = alone();
    let mut running = Running::start("printf 'every line of it\\n'; sleep 30");
    running.wait_for("every line of it", Modifiers::NONE);
    running.chord(Key::Escape, Modifiers::NONE);
    assert_eq!(running.session.selected_text(), None);

    running.right_click(Point::new(120.0, 120.0));
    running.chord(Key::ArrowUp, Modifiers::NONE);
    running.chord(Key::Enter, Modifiers::NONE);
    running.settle();

    let selected = running.session.selected_text().unwrap_or_default();
    assert!(
        selected.contains("every line of it"),
        "select all selected {selected:?}"
    );
}

const WORDS: &str = "self.items.len() (https://ex.com/a_(b)?q=1), ./src/main.rs:12: x";

const WRAPPED: &str = "https://wrapped.example/path";

const WIDE: Size = Size::new(1600.0, 400.0);

#[test]
fn double_click_selects_code_words_and_whole_links() {
    let _alone = alone();
    let mut running = Running::start(&format!(
        "columns() {{ stty size | cut -d' ' -f2; }}; \
         read _; printf '\\033[2J\\033[H%s\\r\\n[%s columns]\\r\\n' '{WORDS}' \"$(columns)\"; \
         read _; printf \"\\033[2J\\033[H%$(($(columns) - 8))s%s\" '' '{WRAPPED}'; sleep 30"
    ));
    running.resize(WIDE);
    running.chord(Key::Enter, Modifiers::NONE);
    let shown = running.wait_for(" columns]", Modifiers::NONE);
    let columns: f32 = shown
        .split('[')
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|count| count.parse().ok())
        .unwrap_or_else(|| panic!("no column count in {shown:?}"));
    let cell = WIDE.width / columns;
    let at = |text: &str, offset: usize| {
        let column = WORDS.find(text).expect("sample should hold the text") + offset;
        Point::new((column as f32 + 0.25) * cell, 0.5)
    };

    for (text, offset, selected) in [
        ("items", 2, "items"),
        ("self", 0, "self"),
        (".len", 0, "."),
        ("len", 1, "len"),
        ("(https", 0, "("),
        ("ex.com", 3, "https://ex.com/a_(b)?q=1"),
        ("q=1", 0, "https://ex.com/a_(b)?q=1"),
        ("main", 1, "./src/main.rs:12"),
        (" x", 1, "x"),
    ] {
        running.double_click(at(text, offset));
        assert_eq!(
            running.session.selected_text().as_deref(),
            Some(selected),
            "double-clicked {text:?} at {offset}"
        );
    }

    running.double_click(at("self", 1));
    running.click(at("len", 1), Modifiers::SHIFT);
    assert_eq!(
        running.session.selected_text().as_deref(),
        Some("self.items.len")
    );

    running.chord(Key::Enter, Modifiers::NONE);
    running.wait_for("example", Modifiers::NONE);
    running.double_click(Point::new(WIDE.width - 1.0, 0.5));
    assert_eq!(
        running.session.selected_text().as_deref(),
        Some(WRAPPED),
        "a link wrapped onto the next row is selected whole"
    );
}

#[test]
fn alt_drag_selects_a_block() {
    let _alone = alone();
    let mut running = Running::start(
        "read _; set -- $(stty size); \
         printf '\\033[2J\\033[Habcdef\\r\\nghijkl\\r\\n[%s %s]\\r\\n' \"$1\" \"$2\"; sleep 30",
    );
    running.chord(Key::Enter, Modifiers::NONE);
    let shown = running.wait_for("]", Modifiers::NONE);
    let size: Vec<f32> = shown
        .split('[')
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .map(|inside| {
            inside
                .split(' ')
                .filter_map(|count| count.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let [lines, columns] = size[..] else {
        panic!("no size in {shown:?}");
    };
    let area = running.session.pixel_size();
    let cell = Size::new(area.width / columns, area.height / lines);
    let at = |column: f32, line: f32| Point::new(column * cell.width, line * cell.height);

    running.drag(at(1.25, 0.5), at(3.75, 1.5), Modifiers::ALT);

    assert_eq!(running.session.selected_text().as_deref(), Some("bcd\nhij"));

    running.drag(at(3.75, 0.5), at(1.25, 1.5), Modifiers::ALT);
    assert_eq!(running.session.selected_text().as_deref(), Some("bcd\nhij"));
    running.chord(Key::ArrowLeft, Modifiers::SHIFT);
    assert_eq!(
        running.session.selected_text().as_deref(),
        Some("abcd\nghij"),
        "the block keeps its anchor corner"
    );
}
