use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_ui::search::Action;
use rikka_widgets::column;
use viewer_media::{Neighbours, Step};

use crate::bar;
use crate::open_file::OpenFile;
use crate::screen::Screen;

#[derive(Debug, State)]
pub(crate) struct Viewer {
    file: OpenFile,
    #[state]
    screen: Screen,
    neighbours: Option<Arc<Neighbours>>,
    navigations: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Navigation {
    Link,
    Neighbour,
}

impl Viewer {
    pub(crate) fn open(file: OpenFile) -> Self {
        Self {
            screen: Screen::open(&file),
            file,
            neighbours: None,
            navigations: 0,
        }
    }

    pub(crate) fn empty() -> Self {
        Self {
            file: OpenFile::empty(),
            screen: Screen::empty(),
            neighbours: None,
            navigations: 0,
        }
    }

    pub(crate) fn start(&mut self) {
        self.list();
        self.screen.start();
    }

    fn list(&self) {
        let Some(kind) = self.file.kind() else {
            return;
        };
        let path = self.file.path().to_path_buf();
        spawn_blocking(
            move || Neighbours::scan(&path, kind),
            |viewer: &mut Viewer, neighbours| viewer.neighbours = Some(Arc::new(neighbours)),
        );
    }

    pub(crate) fn follow(&mut self, target: &str) {
        if self.screen.unsaved() {
            return;
        }
        if let Some(path) = linked(self.file.path(), target) {
            self.visit(path, Navigation::Link);
        }
    }

    pub(crate) fn step(&mut self, step: Step) {
        if self.screen.unsaved() {
            return;
        }
        let Some(neighbours) = &mut self.neighbours else {
            return;
        };
        let Some(path) = neighbours.step(step) else {
            return;
        };
        Arc::make_mut(neighbours).move_to(&path);
        self.visit(path, Navigation::Neighbour);
    }

    fn visit(&mut self, path: PathBuf, visit: Navigation) {
        self.navigations += 1;
        let number = self.navigations;
        spawn_blocking(
            move || OpenFile::open(path),
            move |viewer: &mut Viewer, file: OpenFile| viewer.opened(number, file, visit),
        );
    }

    fn opened(&mut self, number: u64, file: OpenFile, visit: Navigation) {
        if number != self.navigations || self.screen.unsaved() {
            return;
        }
        let missing = file.error().is_some_and(|error| {
            matches!(error.kind(), ErrorKind::NotFound | ErrorKind::IsADirectory)
        });
        if visit == Navigation::Link && missing {
            return;
        }
        self.file = file;
        self.screen = Screen::open(&self.file);
        match visit {
            Navigation::Link => {
                self.neighbours = None;
                self.start();
            }
            Navigation::Neighbour => self.screen.start(),
        }
    }
}

fn linked(page: &Path, target: &str) -> Option<PathBuf> {
    let target = target.split(['#', '?']).next().unwrap_or(target);
    if let Some((scheme, _)) = target.split_once("://") {
        return match scheme.eq_ignore_ascii_case("file") {
            true => rikka_open::uri_to_path(target),
            false => None,
        };
    }
    Some(
        page.parent()
            .unwrap_or(Path::new("."))
            .join(rikka_open::decode_path(target)),
    )
}

pub(crate) fn view(viewer: &Viewer) -> impl View {
    let mut stage = container(
        column! {
            viewer.screen.stage();
            column! {
                viewer.screen.controls();
                bar::status(
                    &viewer.file,
                    viewer.neighbours.as_deref(),
                    viewer.screen.unsaved(),
                    viewer.screen.detail(),
                );
            }
            .gap(Space::SNUG)
            .padding(Inset::xy(12.0, 10.0));
        }
        .fill(),
    )
    .on_command(Command::Save, |screen: &mut Screen| screen.save())
    .on_command(Command::Find, |screen: &mut Screen| screen.open_find(false))
    .on_chord(Key::Char('h'), Modifiers::CONTROL, |screen: &mut Screen| {
        screen.open_find(true)
    })
    .on_command(Command::FindNext, |screen: &mut Screen| {
        screen.find(Action::Step(rikka_ui::search::Step::Next))
    })
    .on_command(Command::FindPrevious, |screen: &mut Screen| {
        screen.find(Action::Step(rikka_ui::search::Step::Previous))
    })
    .on_command(Command::Back, |viewer: &mut Viewer| {
        viewer.step(Step::Previous)
    })
    .on_command(Command::Forward, |viewer: &mut Viewer| {
        viewer.step(Step::Next)
    });
    if viewer.screen.is_finding() {
        stage = stage.on_chord(Key::Escape, Modifiers::NONE, |screen: &mut Screen| {
            screen.find(Action::Close)
        });
    }
    stage.fill()
}
