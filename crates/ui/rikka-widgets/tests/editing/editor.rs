use std::time::{Duration, Instant};

use rikka_renderer::{FrameWork, Scene};
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Rect, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(420.0, 200.0);
const SETTLED: Duration = Duration::from_secs(5);
const PASSES: usize = 3;
const FRAME: Duration = Duration::from_millis(16);
const CARET_WIDTH: f32 = 1.5;
const NEAR: f32 = 0.05;

const CONTROL: Modifiers = Modifiers {
    control: true,
    ..Modifiers::NONE
};
const SHIFT_CONTROL: Modifiers = Modifiers {
    shift: true,
    control: true,
    ..Modifiers::NONE
};

#[derive(State)]
struct Pad {
    document: TextDocument,
    read_only: bool,
    wrap: bool,
    code: bool,
    language: Option<rikka_syntax::Language>,
    edits: usize,
}

fn document_of(text: &str) -> TextDocument {
    let document = TextDocument::new(text);
    document.place_caret(0, false);
    document
}

impl Pad {
    fn new(text: &str) -> Self {
        Self {
            document: document_of(text),
            read_only: false,
            wrap: false,
            code: false,
            language: None,
            edits: 0,
        }
    }

    fn code(text: &str, language: Option<&str>) -> Self {
        Self {
            code: true,
            language: language.and_then(rikka_syntax::Language::named),
            ..Self::new(text)
        }
    }
}

fn pad_view(pad: &Pad) -> Element {
    text_editor(&pad.document)
        .read_only(pad.read_only)
        .wrap(pad.wrap)
        .code(pad.code)
        .language(pad.language)
        .autofocus(true)
        .on_change(|pad: &mut Pad| pad.edits += 1)
        .grow(1.0)
        .build()
}

struct Stage<S: State> {
    ui: TypedUi<S>,
    scene: Scene,
    clock: Instant,
}

impl<S: State> Stage<S> {
    fn new<V: View>(state: S, view: impl Fn(&S) -> V + 'static) -> Self {
        let mut stage = Self {
            ui: TypedUi::new(state, view, Theme::dark()),
            scene: Scene::new(),
            clock: Instant::now(),
        };
        stage.ui.set_viewport(VIEW, 1.0);
        stage.settle();
        stage
    }

    fn settle(&mut self) {
        for _ in 0..PASSES {
            self.clock += SETTLED;
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.clock));
        }
    }

    fn state(&self) -> &S {
        self.ui.state()
    }
    fn press(&mut self, key: Key, modifiers: Modifiers) {
        self.press_within_frame(&[key], modifiers);
    }

    fn press_within_frame(&mut self, keys: &[Key], modifiers: Modifiers) {
        for &key in keys {
            self.ui.handle_event(Event::KeyPressed {
                key,
                code: KeyCode::UNKNOWN,
                location: KeyLocation::Standard,
                modifiers,
            });
        }
        self.settle();
    }

    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            let key = match character {
                ' ' => Key::Space,
                '\n' => Key::Enter,
                character => Key::Char(character),
            };
            self.press(key, Modifiers::NONE);
        }
    }

    fn caret(&self) -> Rect {
        self.ui.caret().expect("focused caret")
    }

    fn drawn_caret(&self) -> Option<Point> {
        self.ui.dump().lines().find_map(|line| {
            let rect = line.trim().strip_prefix("· quad [")?;
            let (x, rest) = rect.split_once(' ')?;
            let (y, size) = rest.split_once(' ')?;
            let centre = x.parse::<f32>().ok()? + CARET_WIDTH / 2.0;
            size.starts_with("1.50x")
                .then_some(Point::new(centre, y.parse().ok()?))
        })
    }

    fn next_frame(&mut self) {
        self.clock += FRAME;
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.clock));
    }

    fn update(&mut self, update: impl FnOnce(&mut S)) {
        self.ui.update(update);
    }

    fn frame(&mut self) {
        self.clock += SETTLED;
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.clock));
    }
}

impl Stage<Pad> {
    fn pad(state: Pad) -> Self {
        Self::new(state, pad_view)
    }

    fn document(&self) -> TextDocument {
        self.state().document.clone()
    }
}

#[test]
fn typing_edits_document_once_per_key() {
    let mut harness = Stage::pad(Pad::new(""));
    harness.type_text("Hi あ");

    assert_eq!(harness.document().text(), "Hi あ");
    assert_eq!(harness.state().edits, 4, "expected one report per edit");
}

#[test]
fn newline_keeps_indent_and_undoes_burst() {
    let mut harness = Stage::pad(Pad::new("    one"));
    harness.press(Key::End, Modifiers::NONE);
    harness.press(Key::Enter, Modifiers::NONE);
    harness.type_text("two");

    assert_eq!(harness.document().text(), "    one\n    two");

    harness.press(Key::Char('z'), CONTROL);
    assert_eq!(harness.document().text(), "    one\n    ");
    harness.press(Key::Char('z'), CONTROL);
    assert_eq!(harness.document().text(), "    one");
    harness.press(Key::Char('z'), SHIFT_CONTROL);
    assert_eq!(harness.document().text(), "    one\n    ");
}

#[test]
fn read_only_editor_moves_caret_only() {
    let mut harness = Stage::pad(Pad {
        read_only: true,
        ..Pad::new("one\ntwo")
    });
    harness.type_text("x");
    assert_eq!(harness.document().text(), "one\ntwo");

    harness.press(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(harness.document().caret_line_column().0, 1);

    harness.press(Key::Char('a'), CONTROL);
    assert_eq!(harness.document().selected_text(), "one\ntwo");
}

#[test]
fn keys_within_one_frame_apply_in_order() {
    let mut harness = Stage::pad(Pad::new("one\ntwo\nthree"));
    harness.press_within_frame(
        &[
            Key::ArrowDown,
            Key::Char('x'),
            Key::Home,
            Key::ArrowDown,
            Key::Char('y'),
        ],
        Modifiers::NONE,
    );
    assert_eq!(harness.document().text(), "one\nxtwo\nythree");
}

#[derive(State)]
struct Reopening {
    document: TextDocument,
    place: Place,
    away: bool,
}

fn reopening_view(reopening: &Reopening) -> Element {
    if reopening.away {
        return container(spacer()).fill().build();
    }
    text_editor(&reopening.document)
        .wrap(false)
        .autofocus(true)
        .place(&reopening.place)
        .on_change(|| {})
        .grow(1.0)
        .build()
}

#[test]
fn reopened_file_restores_scroll() {
    let mut stage = Stage::new(
        Reopening {
            document: document_of(&"line\n".repeat(200)),
            place: Place::new(),
            away: false,
        },
        reopening_view,
    );
    let top = stage.caret().y();
    stage.press(Key::End, CONTROL);
    let read = stage.caret().y();
    assert!(read > top, "view did not reach end of file");

    stage.update(|reopening| reopening.away = true);
    stage.settle();
    stage.update(|reopening| reopening.away = false);
    stage.frame();

    assert_eq!(
        stage.caret().y(),
        read,
        "reopened file lost scroll position"
    );
}

#[test]
fn brackets_auto_close_and_skip() {
    let mut harness = Stage::pad(Pad::code("", None));
    harness.type_text("call(");
    assert_eq!(harness.document().text(), "call()");
    harness.type_text("x)");
    assert_eq!(harness.document().text(), "call(x)");
    assert_eq!(harness.document().caret(), 7);
}

#[test]
fn ctrl_slash_toggles_line_comment() {
    let mut harness = Stage::pad(Pad::code("let a = 1;", Some("rust")));
    harness.press(Key::Char('/'), CONTROL);
    assert_eq!(harness.document().text(), "// let a = 1;");
    harness.press(Key::Char('/'), CONTROL);
    assert_eq!(harness.document().text(), "let a = 1;");
}

#[test]
fn scrolling_reuses_laid_text() {
    let text: String = (0..2_000)
        .map(|line| format!("line number {line}\n"))
        .collect();
    let mut harness = Stage::pad(Pad::new(&text));
    let mut renderer = crate::common::renderer();
    let mut target = renderer
        .create_target(VIEW.width as u32, VIEW.height as u32)
        .unwrap();
    let mut scroll = |harness: &mut Stage<Pad>| {
        harness.ui.handle_event(Event::Scroll {
            position: Point::new(VIEW.width / 2.0, VIEW.height / 2.0),
            delta: ScrollDelta::from_offset(Point::new(0.0, 24.0)),
            modifiers: Modifiers::NONE,
            source: ScrollSource::Wheel,
        });
        let before = renderer.frame_work();
        for _ in 0..12 {
            harness.clock += FRAME;
            harness
                .ui
                .render(Rendering::new(&mut harness.scene).with_time(harness.clock));
            renderer.draw(&mut harness.scene, &mut target).unwrap();
        }
        renderer.frame_work().since(before)
    };
    let warmed = scroll(&mut harness);
    let mut work = FrameWork::default();
    for _ in 0..3 {
        work.add(scroll(&mut harness));
    }

    assert!(warmed.uploaded_instances > 100, "text not drawn");
    assert!(work.frames >= 3, "scrolls missing");
    assert!(
        work.uploaded_instances < 60,
        "scrolling resent {} instances",
        work.uploaded_instances
    );
}

#[test]
fn caret_glides_and_reports_final_rect() {
    let text: String = (0..60).map(|line| format!("line {line}\n")).collect();
    let mut stage = Stage::pad(Pad::new(&text));
    let start = stage.caret().origin;
    for key in [Key::ArrowDown, Key::ArrowDown, Key::ArrowDown, Key::End] {
        stage.ui.handle_event(Event::KeyPressed {
            key,
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers: Modifiers::NONE,
        });
    }
    stage.next_frame();
    let reported = stage.caret().origin;
    let mut path = Vec::new();
    for _ in 0..12 {
        stage.next_frame();
        path.push(stage.drawn_caret().expect("caret drawn"));
    }
    let final_point = path[path.len() - 1];

    assert!(
        reported.x > start.x && reported.y > start.y,
        "caret moved: {start:?} → {reported:?}"
    );
    assert!(
        path.iter().any(|at| start.x < at.x
            && at.x < reported.x - NEAR
            && start.y < at.y
            && at.y < reported.y - NEAR),
        "caret jumped from {start:?} to {reported:?}: {path:?}"
    );
    assert!(
        (final_point.x - reported.x).abs() < NEAR && (final_point.y - reported.y).abs() < NEAR,
        "caret reported at {reported:?}, settled at {final_point:?}"
    );

    stage.ui.handle_event(Event::Scroll {
        position: Point::new(VIEW.width / 2.0, VIEW.height / 2.0),
        delta: ScrollDelta::from_offset(Point::new(0.0, 24.0)),
        modifiers: Modifiers::NONE,
        source: ScrollSource::Wheel,
    });
    for frame in 0..12 {
        stage.next_frame();
        let drawn = stage.drawn_caret().expect("caret drawn");
        assert!(
            (drawn.x - final_point.x).abs() < NEAR && (drawn.y - final_point.y).abs() < NEAR,
            "frame {frame}: scroll moved caret to {drawn:?} from {final_point:?}"
        );
    }
    assert!(
        stage.caret().y() < reported.y - 1.0,
        "scroll did not move text and caret together"
    );
}
