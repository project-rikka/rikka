use std::time::{Duration, Instant};

use crate::common::dump_rect;
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Rect, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(480.0, 200.0);
const SETTLED: Duration = Duration::from_secs(5);
const PASSES: usize = 3;
const NUDGE: Point = Point::new(0.5, 0.0);
const PROSE_INSET: f32 = 4.0;
const TEXT: &str = "alpha beta gamma\nlast line";
const PROSE: &str = "alpha beta gamma";
const BETA: usize = 6;
const GAMMA: usize = 11;
const LAST: usize = 17;

const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};
const CONTROL: Modifiers = Modifiers {
    control: true,
    ..Modifiers::NONE
};

#[derive(State)]
struct Sheet {
    document: TextDocument,
}

fn editor_view(sheet: &Sheet) -> Element {
    text_editor(&sheet.document)
        .autofocus(true)
        .grow(1.0)
        .build()
}

fn prose_view(_: &Sheet) -> Element {
    column! {
        rich_text(PROSE).selectable("prose");
    }
    .padding(Space::WIDE)
    .fill()
    .build()
}

struct Harness {
    ui: TypedUi<Sheet>,
    scene: Scene,
    clock: Instant,
}

impl Harness {
    fn new(view: fn(&Sheet) -> Element) -> Self {
        let document = TextDocument::new(TEXT);
        document.place_caret(0, false);
        let mut harness = Self {
            ui: TypedUi::new(Sheet { document }, view, Theme::dark()),
            scene: Scene::new(),
            clock: Instant::now(),
        };
        harness.ui.set_viewport(VIEW, 1.0);
        harness.settle();
        harness
    }

    fn settle(&mut self) {
        for _ in 0..PASSES {
            self.clock += SETTLED;
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.clock));
        }
    }

    fn document(&self) -> TextDocument {
        self.ui.state().document.clone()
    }

    fn boundary(&mut self, offset: usize) -> Point {
        self.document().place_caret(offset, false);
        self.settle();
        self.ui.caret().expect("focused caret").center()
    }

    fn within(&mut self, character: usize, share: f32) -> Point {
        let start = self.boundary(character);
        let end = self.boundary(character + 1);
        let at = Point::new(start.x + (end.x - start.x) * share, start.y);
        self.document().place_caret(0, false);
        self.settle();
        at
    }

    fn pressed(&mut self, at: Point, modifiers: Modifiers) {
        self.ui.handle_event(Event::PointerMoved(at));
        self.ui.handle_event(Event::PointerPressed {
            position: at,
            button: PointerButton::Primary,
            modifiers,
        });
    }

    fn released(&mut self, at: Point) {
        self.ui.handle_event(Event::PointerReleased {
            position: at,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        });
    }

    fn clicks(&mut self, at: Point, count: usize) {
        for _ in 0..count {
            self.pressed(at, Modifiers::NONE);
            self.released(at);
        }
        self.settle();
    }

    fn moved(&mut self, to: Point) {
        self.ui.handle_event(Event::PointerMoved(to));
        self.settle();
    }

    fn selected(&self) -> String {
        self.document().selected_text()
    }

    fn copied(&mut self) -> Option<String> {
        self.ui.handle_event(Event::KeyPressed {
            key: Key::Char('c'),
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers: CONTROL,
        });
        self.settle();
        clipboard::text()
    }

    fn prose(&self) -> Rect {
        let dump = self.ui.dump();
        dump.lines()
            .find(|line| line.trim_start().starts_with("RichText ["))
            .map(dump_rect)
            .unwrap_or_else(|| panic!("no RichText in\n{dump}"))
    }
}

#[test]
fn double_click_selects_the_word_under_the_pointer() {
    for (character, share, word) in [
        (BETA + 1, 0.5, "beta"),
        (BETA + 3, 0.75, "beta"),
        (BETA, 0.25, "beta"),
        (GAMMA - 1, 0.25, " "),
        (GAMMA, 0.25, "gamma"),
        (BETA - 1, 0.75, " "),
    ] {
        let mut harness = Harness::new(editor_view);
        let at = harness.within(character, share);
        harness.clicks(at, 2);
        assert_eq!(harness.selected(), word, "{character} at {share}");
    }
}

#[test]
fn dragging_after_double_click_extends_by_whole_words() {
    let mut harness = Harness::new(editor_view);
    let beta = harness.within(BETA + 1, 0.5);
    let gamma = harness.within(GAMMA + 2, 0.5);
    let alpha = harness.within(1, 0.5);

    harness.clicks(beta, 1);
    harness.pressed(beta, Modifiers::NONE);
    harness.settle();
    assert_eq!(harness.selected(), "beta");

    harness.moved(beta + NUDGE);
    assert_eq!(harness.selected(), "beta", "a still pointer broke the word");

    harness.moved(gamma);
    assert_eq!(harness.selected(), "beta gamma");

    harness.moved(alpha);
    assert_eq!(harness.selected(), "alpha beta");
    let document = harness.document();
    assert_eq!((document.anchor(), document.caret()), (GAMMA - 1, 0));

    harness.moved(beta);
    harness.released(beta);
    harness.settle();
    assert_eq!(harness.selected(), "beta");
}

#[test]
fn shift_click_after_double_click_extends_by_whole_words() {
    let mut harness = Harness::new(editor_view);
    let beta = harness.within(BETA + 1, 0.5);
    let gamma = harness.within(GAMMA + 2, 0.5);

    harness.clicks(beta, 2);
    harness.pressed(gamma, SHIFT);
    harness.released(gamma);
    harness.settle();
    assert_eq!(harness.selected(), "beta gamma");
}

#[test]
fn triple_click_selects_the_whole_line() {
    for (character, line) in [(BETA + 1, "alpha beta gamma\n"), (LAST + 2, "last line")] {
        let mut harness = Harness::new(editor_view);
        let at = harness.within(character, 0.5);
        harness.clicks(at, 3);
        assert_eq!(harness.selected(), line, "{character}");

        harness.moved(at + NUDGE);
        assert_eq!(harness.selected(), line, "{character} after a nudge");
    }
}

#[test]
fn selectable_text_keeps_double_clicked_word_while_dragging() {
    let mut harness = Harness::new(prose_view);
    let prose = harness.prose();
    let start = Point::new(prose.x() + PROSE_INSET, prose.center().y);
    let end = Point::new(prose.max_x() + PROSE_INSET, start.y);

    harness.clicks(start, 1);
    harness.pressed(start, Modifiers::NONE);
    harness.moved(start + NUDGE);
    let word = harness.copied().expect("copied word");
    assert!(
        PROSE.split(' ').any(|whole| whole == word),
        "{word:?} is not a whole word"
    );

    harness.moved(end);
    harness.released(end);
    harness.settle();
    let reached = harness.copied().expect("copied words");
    assert!(
        reached.starts_with(&word) && PROSE.ends_with(&reached),
        "{reached:?} from {word:?}"
    );
}
