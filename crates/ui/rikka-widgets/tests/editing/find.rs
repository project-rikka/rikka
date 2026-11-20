use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::search::{Action, Rule, Search, Step, Tally};
use rikka_ui::*;
use rikka_widgets::*;

use crate::common::{Wakes, wakes};

const VIEW: Size = Size::new(420.0, 420.0);
const SETTLED: Duration = Duration::from_secs(5);
const SCAN_TIMEOUT: Duration = Duration::from_secs(5);
const FIND: &str = "find";
const WRITING: &str = "one two one\nthree one";

#[derive(State)]
struct Writing {
    document: TextDocument,
    search: Search,
}

fn view(writing: &Writing) -> Element {
    column_of([
        find_mark(FIND),
        text_editor(&writing.document)
            .search(&writing.search)
            .grow(1.0)
            .build(),
        find_bar(&writing.search)
            .anchored_to(FIND)
            .replaceable(true)
            .on_action(|writing: &mut Writing, action: Action| writing.search.act(action))
            .build(),
    ])
    .fill()
    .build()
}
fn writing(pattern: &str, open: bool) -> Writing {
    let search = Search::new();
    if open {
        search.open(None);
    }
    if !pattern.is_empty() {
        search.seek(pattern);
    }
    Writing {
        document: TextDocument::new(WRITING),
        search,
    }
}

struct Reading {
    ui: TypedUi<Writing>,
    scene: Scene,
    now: Instant,
    document: TextDocument,
    search: Search,
    woken: Wakes,
}

impl Reading {
    fn open(pattern: &str) -> Self {
        let state = writing("", true);
        let (document, search) = (state.document.clone(), state.search.clone());
        let mut ui = TypedUi::new(state, view, Theme::dark());
        let woken = wakes(&mut ui);
        let mut reading = Self {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
            document,
            search,
            woken,
        };
        reading.ui.set_viewport(VIEW, 1.0);
        reading.draw();
        if !pattern.is_empty() {
            reading.search.seek(pattern);
            reading.draw();
        }
        reading
    }

    fn draw(&mut self) {
        loop {
            self.now += SETTLED;
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.now));
            if !self.search.is_looking() {
                return;
            }
            assert!(self.woken.wait(SCAN_TIMEOUT), "search scan did not finish");
        }
    }

    fn act(&mut self, action: Action) {
        self.search.act(action);
        self.draw();
    }

    fn tally(&self) -> Tally {
        self.search.tally()
    }

    fn selected(&self) -> String {
        self.document.selected_text()
    }
}

#[test]
fn find_reports_match_count() {
    let reading = Reading::open("one");

    assert_eq!(reading.tally(), Tally::new(3, Some(0)));
}

#[test]
fn replace_overwrites_current_match() {
    let mut reading = Reading::open("one");
    reading.search.set_replacement("four");

    reading.act(Action::Step(Step::Replace));

    assert_eq!(reading.document.text(), "four two one\nthree one");
    assert_eq!(reading.tally().found, 2);
    assert_eq!(reading.selected(), "one", "next match not selected");
}

#[test]
fn replace_all_undoes_as_one_step() {
    let mut reading = Reading::open("one");
    reading.search.set_replacement("four");

    reading.act(Action::Step(Step::ReplaceAll));
    assert_eq!(reading.document.text(), "four two four\nthree four");
    assert_eq!(reading.tally(), Tally::new(0, None));

    reading.ui.update(|writing| {
        writing.document.undo();
    });
    reading.draw();
    assert_eq!(reading.document.text(), WRITING);
    assert_eq!(reading.tally().found, 3);
}

#[test]
fn regex_captures_expand_in_replacement() {
    let mut reading = Reading::open("(o)ne");
    reading.act(Action::Rule(Rule::Regex));
    reading.search.set_replacement("[$1]");

    reading.act(Action::Step(Step::ReplaceAll));

    assert_eq!(reading.document.text(), "[o] two [o]\nthree [o]");
}
