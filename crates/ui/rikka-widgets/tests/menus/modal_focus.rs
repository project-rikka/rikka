use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(480.0, 320.0);
const FRAME: Duration = Duration::from_millis(16);
const FRAMES: usize = 30;

#[derive(State)]
struct Desk {
    outside: TextDocument,
    inside: TextDocument,
    open: bool,
    sheet: bool,
    chords: usize,
}

fn desk_view(desk: &Desk) -> Element {
    let body = column! {
        text_field(&desk.outside).autofocus(true);
        if desk.open {
            popover(text_field(&desk.inside).autofocus(true).claims_focus(true))
                .on_dismiss(|desk: &mut Desk| desk.open = false);
        }
        if desk.sheet {
            sheet(column! {
                button(text("One"));
                button(text("Two"));
            })
            .on_dismiss(|desk: &mut Desk| desk.sheet = false);
        }
    };
    container(body)
        .on_chord(Key::Char('k'), Modifiers::CONTROL, |desk: &mut Desk| {
            desk.chords += 1;
        })
        .build()
}

struct Stage {
    ui: TypedUi<Desk>,
    scene: Scene,
    now: Instant,
}

impl Stage {
    fn new() -> Stage {
        let desk = Desk {
            outside: TextDocument::new(""),
            inside: TextDocument::new(""),
            open: false,
            sheet: false,
            chords: 0,
        };
        let mut ui = TypedUi::new(desk, desk_view, Theme::dark());
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

    fn write(&mut self, text: &str) {
        self.ui.handle_event(Event::Commit(text.to_owned()));
        self.settle();
    }

    fn press(&mut self, key: Key) {
        self.chord(key, Modifiers::NONE);
    }

    fn chord(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.handle_event(Event::KeyPressed {
            key,
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers,
        });
        self.settle();
    }
}

#[test]
fn modal_field_takes_and_restores_focus() {
    let mut stage = Stage::new();
    stage.write("a");
    assert_eq!(stage.ui.state().outside.text(), "a");

    stage.ui.update(|desk| desk.open = true);
    stage.settle();
    stage.write("b");
    assert_eq!(stage.ui.state().inside.text(), "b");
    assert_eq!(stage.ui.state().outside.text(), "a");

    stage.press(Key::Escape);
    assert!(!stage.ui.state().open);
    stage.write("c");
    assert_eq!(stage.ui.state().outside.text(), "ac");
}

#[test]
fn sheet_holds_keyboard_until_dismissed() {
    let mut stage = Stage::new();
    stage.chord(Key::Char('k'), Modifiers::CONTROL);
    assert_eq!(stage.ui.state().chords, 1);

    stage.ui.update(|desk| desk.sheet = true);
    stage.settle();
    stage.write("a");
    stage.press(Key::Tab);
    stage.press(Key::Tab);
    stage.press(Key::Tab);
    stage.write("b");
    stage.chord(Key::Char('k'), Modifiers::CONTROL);
    assert_eq!(stage.ui.state().outside.text(), "");
    assert_eq!(stage.ui.state().chords, 1);

    stage.press(Key::Escape);
    assert!(!stage.ui.state().sheet);
    stage.write("c");
    stage.chord(Key::Char('k'), Modifiers::CONTROL);
    assert_eq!(stage.ui.state().outside.text(), "c");
    assert_eq!(stage.ui.state().chords, 2);
}

#[derive(State)]
struct Filing {
    sheet: bool,
    confirmed: usize,
    chords: usize,
}

fn filing_view(filing: &Filing) -> Element {
    let body = column! {
        if filing.sheet {
            sheet(
                column! {
                    button(text("Confirm")).on_press(|filing: &mut Filing| filing.confirmed += 1);
                }
                .padding(20.0),
            )
            .width(200.0)
            .on_dismiss(|filing: &mut Filing| filing.sheet = false);
        }
    }
    .fill();
    column! {
        menu_bar().menu(
            "File",
            [menu_item("New").on_press(|filing: &mut Filing| filing.sheet = true)],
        );
        container(body)
            .on_chord(Key::Char('k'), Modifiers::CONTROL, |filing: &mut Filing| {
                filing.chords += 1;
            })
            .grow(1.0)
            .fill_width();
    }
    .fill()
    .build()
}

struct Filer {
    ui: TypedUi<Filing>,
    scene: Scene,
    now: Instant,
}

impl Filer {
    fn new() -> Filer {
        let filing = Filing {
            sheet: false,
            confirmed: 0,
            chords: 0,
        };
        let mut ui = TypedUi::new(filing, filing_view, Theme::dark());
        ui.set_viewport(VIEW, 1.0);
        let mut filer = Filer {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
        };
        filer.settle();
        filer
    }

    fn settle(&mut self) {
        for _ in 0..FRAMES {
            self.now += FRAME;
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.now));
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        let position = Point::new(x, y);
        let button = PointerButton::Primary;
        let modifiers = Modifiers::NONE;
        for event in [
            Event::PointerMoved(position),
            Event::PointerPressed {
                position,
                button,
                modifiers,
            },
            Event::PointerReleased {
                position,
                button,
                modifiers,
            },
        ] {
            self.ui.handle_event(event);
            self.settle();
        }
    }

    fn chord(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.handle_event(Event::KeyPressed {
            key,
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers,
        });
        self.settle();
    }
}

#[test]
fn sheet_opened_from_menu_bar_takes_pointer_and_hands_keys_back() {
    let mut filer = Filer::new();
    filer.click(15.0, 15.0);
    filer.click(40.0, 51.0);
    assert!(filer.ui.state().sheet);

    filer.click(40.0, 68.0);
    assert_eq!(filer.ui.state().confirmed, 1);

    filer.chord(Key::Escape, Modifiers::NONE);
    assert!(!filer.ui.state().sheet);
    filer.chord(Key::Char('k'), Modifiers::CONTROL);
    assert_eq!(filer.ui.state().chords, 1);
}
