use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Rect, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(400.0, 400.0);
const TALL: f32 = 200.0;
const SHORT: f32 = 60.0;
const BUTTON: f32 = 40.0;
const SETTLED: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Seen {
    drawn: usize,
    at: Rect,
    shown: Rect,
}

#[derive(Clone, Default)]
struct Probe {
    seen: Rc<Cell<Seen>>,
    grows: bool,
}

impl Probe {
    fn seen(&self) -> Seen {
        self.seen.get()
    }
}

impl Widget for Probe {
    type State = ();

    fn layout(&self) -> Layout {
        let block = Layout::column().width(Length::FILL);
        match self.grows {
            true => block.grow(1.0),
            false => block,
        }
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        self.seen.set(Seen {
            drawn: self.seen.get().drawn + 1,
            at: ctx.bounds(),
            shown: ctx.visible(),
        });
    }
}

#[derive(State)]
struct Book {
    at: usize,
    pages: [Probe; 3],
    pressed: Rc<Cell<usize>>,
}

fn book(book: &Book) -> impl View {
    page_stack(book.at, book.page()).fill_width()
}

impl Book {
    fn page(&self) -> Element {
        let block = Element::new(self.pages[self.at].clone()).height(Length::px(match self.at {
            0 => TALL,
            _ => SHORT,
        }));
        match self.at {
            0 => column! {
                block;
                button(text("press"))
                    .on_press(|book: &mut Book| book.pressed.set(book.pressed.get() + 1))
                    .height(Length::px(BUTTON))
                    .fill_width()
            }
            .fill_width()
            .build(),
            _ => column! { block }.fill_width().build(),
        }
    }
}

struct Reading {
    ui: TypedUi<Book>,
    scene: Scene,
    pages: [Probe; 3],
    now: Instant,
}

impl Reading {
    fn draws(&mut self, elapsed: Duration) {
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.now + elapsed));
    }

    fn turns(&mut self, at: usize, elapsed: Duration) {
        self.ui.update(|book| book.at = at);
        self.draws(elapsed);
    }
}

fn reading() -> Reading {
    let pages = [Probe::default(), Probe::default(), Probe::default()];
    let pressed = Rc::new(Cell::new(0));
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Book {
            at: 0,
            pages: pages.clone(),
            pressed: Rc::clone(&pressed),
        },
        book,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    ui.render(Rendering::new(&mut scene).with_time(now + SETTLED));
    Reading {
        ui,
        scene,
        pages,
        now,
    }
}

#[test]
fn departed_page_is_removed() {
    let mut reading = reading();

    reading.turns(1, Duration::ZERO);
    reading.draws(Duration::from_millis(400));
    reading.draws(Duration::from_millis(500));
    let departed_page = reading.pages[0].seen();
    reading.draws(Duration::from_millis(600));
    reading.draws(Duration::from_millis(700));

    assert_eq!(
        reading.pages[0].seen().drawn,
        departed_page.drawn,
        "departed page still drawn"
    );
}

#[derive(State)]
struct Chapter {
    at: Tracked<usize>,
    line: Tracked<usize>,
}

fn chapter(chapter: &Chapter) -> impl View {
    page_stack(
        *chapter.at,
        component(|chapter: &Chapter| chapter, chapter_view),
    )
    .fill()
}

fn chapter_view(chapter: &Chapter) -> impl View {
    let (at, line) = (*chapter.at, *chapter.line);
    lazy_column(1, move |_| text(format!("chapter {at} line {line}"))).fill()
}

struct Telling {
    ui: TypedUi<Chapter>,
    scene: Scene,
    now: Instant,
}

impl Telling {
    fn new() -> Telling {
        let mut telling = Telling {
            ui: TypedUi::new(
                Chapter {
                    at: Tracked::new(0),
                    line: Tracked::new(0),
                },
                chapter,
                Theme::dark(),
            ),
            scene: Scene::new(),
            now: Instant::now(),
        };
        telling.ui.set_viewport(VIEW, 1.0);
        telling.draws(Duration::ZERO);
        telling.draws(SETTLED);
        telling
    }

    fn draws(&mut self, elapsed: Duration) {
        self.now += elapsed;
        let at = self.now;
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(at));
    }

    fn frames(&mut self, count: usize) {
        for _ in 0..count {
            self.draws(Duration::from_millis(16));
        }
    }
}

#[test]
fn outgoing_page_keeps_component_output() {
    let mut telling = Telling::new();

    telling.ui.start(|chapter: &mut Chapter| chapter.at.set(1));
    telling.frames(5);

    let shown = telling.ui.dump();
    assert!(
        shown.contains("chapter 0 line 0") && shown.contains("chapter 1 line 0"),
        "outgoing page rebuilt:\n{shown}"
    );
}

#[test]
fn current_page_shows_latest_component_output() {
    let mut telling = Telling::new();

    telling
        .ui
        .start(|chapter: &mut Chapter| chapter.line.set(1));
    telling.frames(2);

    let shown = telling.ui.dump();
    assert!(
        shown.contains("chapter 0 line 1"),
        "current page kept stale content:\n{shown}"
    );
}
