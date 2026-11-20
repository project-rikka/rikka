use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_syntax::Language;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(480.0, 320.0);
const SETTLED: Duration = Duration::from_secs(5);
const PASSES: usize = 3;

const PROGRAM: &str = "\
fn main() {
    let greeting = \"hello\";
    println!(\"{greeting}\");
}
";

#[derive(State)]
struct Pad {
    document: TextDocument,
    language: Option<Language>,
}

fn editing(pad: &Pad) -> impl View {
    text_editor(&pad.document)
        .language(pad.language)
        .autofocus(true)
        .on_change(|| {})
        .grow(1.0)
}

struct Stage {
    ui: TypedUi<Pad>,
    scene: Scene,
    clock: Instant,
}

impl Stage {
    fn new(text: &str, language: Option<Language>) -> Stage {
        let document = TextDocument::new(text);
        document.place_caret(0, false);
        let mut stage = Stage {
            ui: TypedUi::new(Pad { document, language }, editing, Theme::dark()),
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

    fn type_text(&mut self, text: &str) {
        for letter in text.chars() {
            let key = match letter {
                ' ' => Key::Space,
                '\n' => Key::Enter,
                letter => Key::Char(letter),
            };
            self.ui.handle_event(Event::KeyPressed {
                key,
                code: KeyCode::UNKNOWN,
                location: KeyLocation::Standard,
                modifiers: Modifiers::NONE,
            });
        }
        self.settle();
    }

    fn text(&self) -> String {
        self.ui.state().document.text()
    }
}

#[test]
fn editor_highlights_long_file_incrementally() {
    let long = "fn main() { let x = 1; }\n".repeat(20_000);
    let mut stage = Stage::new(&long, Language::named("rust"));

    stage.type_text("//");

    assert!(stage.text().starts_with("//fn main()"));
}

#[test]
fn language_change_keeps_text() {
    let mut stage = Stage::new(PROGRAM, Language::named("rust"));

    stage
        .ui
        .update(|pad| pad.language = Language::named("python"));
    stage.settle();

    assert_eq!(stage.ui.state().language, Language::named("python"));
    assert_eq!(stage.text(), PROGRAM);
}
