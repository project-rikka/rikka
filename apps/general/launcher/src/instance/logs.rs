use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use launcher_game::{LevelTracker, LogLine};
use launcher_instance::{LogFile, LogKind};
use rikka_app::prelude::*;
use rikka_ui::{Element, component_with};
use rikka_widgets::{column, row};

use crate::launcher::Launcher;
use crate::load::{Load, loading};
use crate::logs::{self, Lines};
use crate::widgets::labelled;

#[derive(Default)]
pub(crate) struct LogState {
    pub(crate) files: Load<Vec<LogFile>>,
    pub(crate) shown: usize,
    pub(crate) lines: Load<Lines>,
}

fn parsed(text: &str) -> Vec<LogLine> {
    let mut tracker = LevelTracker::default();
    text.lines()
        .map(|line| tracker.line(line.to_owned()))
        .collect()
}

impl Launcher {
    pub(crate) fn load_log_files(&mut self) {
        let (Some(id), Some(services)) = (self.selected_id(), self.services()) else {
            return;
        };
        spawn_blocking_keyed(
            "launcher-log-files",
            move || services.instances.get(&id)?.logs(),
            |launcher: &mut Launcher, listed: launcher_instance::Result<Vec<LogFile>>| {
                launcher.instance.logs.files = match listed {
                    Ok(listed) => Load::Ready(listed),
                    Err(error) => Load::Failed(error.to_string()),
                };
            },
        );
    }

    fn show_log(&mut self, shown: usize) {
        self.instance.logs.shown = shown;
        let Some(path) = shown
            .checked_sub(1)
            .and_then(|index| self.instance.logs.files.ready()?.get(index))
            .map(|file| file.path.clone())
        else {
            return;
        };
        self.instance.logs.lines = Load::Loading;
        spawn_blocking_keyed(
            "launcher-log-text",
            move || launcher_instance::read_log(&path).map(|text| parsed(&text)),
            |launcher: &mut Launcher, read: launcher_instance::Result<Vec<LogLine>>| {
                launcher.instance.logs.lines = match read {
                    Ok(lines) => Load::Ready(Rc::new(RefCell::new(VecDeque::from(lines)))),
                    Err(error) => Load::Failed(error.to_string()),
                };
            },
        );
    }

    fn open_logs_folder(&mut self) {
        let Some(path) = self
            .selected
            .as_deref()
            .and_then(|id| self.instance(id))
            .map(|instance| instance.path.join("logs"))
        else {
            return;
        };
        let folder: PathBuf = match path.is_dir() {
            true => path,
            false => path.parent().map(PathBuf::from).unwrap_or(path),
        };
        if let Err(error) = rikka_open::open(&folder.display().to_string()) {
            self.fail(error.to_string());
        }
    }
}

fn choice(file: &LogFile) -> Label {
    match file.kind {
        LogKind::Game => Label::from(file.name.clone()),
        LogKind::Crash => tr!("launcher.logs.crash", name = file.name.as_str()),
    }
}

pub(super) fn view(launcher: &Launcher, id: &str) -> Element {
    let state = &launcher.instance.logs;
    let mut choices = vec![tr!("launcher.logs.live")];
    if let Some(files) = state.files.ready() {
        choices.extend(files.iter().map(choice));
    }
    let toolbar = row! {
        combo(Some(state.shown.min(choices.len().saturating_sub(1))))
            .options(choices)
            .on_select(Launcher::show_log)
            .width(Length::px(320.0));
        spacer().grow(1.0);
        button(labelled(MaterialSymbol::REFRESH, tr!("launcher.logs.refresh")))
            .ghost()
            .on_press(Launcher::load_log_files);
        button(labelled(MaterialSymbol::FOLDER_OPEN, tr!("launcher.logs.folder")))
            .ghost()
            .on_press(Launcher::open_logs_folder);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::WIDE, Space::NONE))
    .fill_width();
    let body = match state.shown {
        0 => component_with(
            |launcher: &Launcher| &launcher.logs,
            std::sync::Arc::<str>::from(id),
            logs::live,
        ),
        _ => loading(&state.lines, |lines: &Lines| {
            logs::log_list(Rc::clone(lines))
        }),
    };
    column! {
        toolbar;
        container(
            container(body)
                .surface()
                .radius(Corner::Panel)
                .border(1.0, palette().outline)
                .clip_content()
                .fill(),
        )
        .padding(Inset::edges(Space::NONE, Space::WIDE, Space::WIDE, Space::WIDE))
        .grow(1.0)
        .fill_width();
    }
    .gap(Space::NORMAL)
    .fill()
    .any()
}
