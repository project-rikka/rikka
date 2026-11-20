use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use ade_git::{Branch, Kind, Worktree};
use rikka_app::prelude::*;
use rikka_chat::question_chip;
use rikka_widgets::{column, row};

use crate::labels;
use crate::notice::notice;
use crate::project::act::{act, given, given_with_setup};

const PICKER: f32 = 176.0;

#[derive(Debug)]
pub(crate) struct Offer {
    path: PathBuf,
    label: String,
}

impl Offer {
    pub(crate) fn new(path: PathBuf, label: String) -> Offer {
        Offer { path, label }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug)]
pub(crate) struct Making {
    branch: TextDocument,
    base: Option<String>,
    busy: bool,
    error_message: Option<Arc<str>>,
}

impl Making {
    pub(crate) fn new(base: Option<String>) -> Making {
        Making {
            branch: TextDocument::new(""),
            base,
            busy: false,
            error_message: None,
        }
    }

    pub(crate) fn branch(&self) -> String {
        self.branch.text().trim().to_owned()
    }

    pub(crate) fn base(&self) -> Option<&str> {
        self.base.as_deref()
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.busy
    }

    pub(crate) fn has_error(&self) -> bool {
        self.error_message.is_some()
    }

    pub(crate) fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub(crate) fn set_base(&mut self, base: String) {
        self.base = Some(base);
    }

    pub(crate) fn start(&mut self) {
        self.busy = true;
        self.error_message = None;
    }

    pub(crate) fn report_error(&mut self, message: Arc<str>) {
        self.busy = false;
        self.error_message = Some(message);
    }
}

pub(super) struct Choosing<'a> {
    pub(super) offers: &'a [Offer],
    pub(super) at: Option<usize>,
    pub(super) locked: bool,
}

pub(super) struct Naming<'a> {
    pub(super) making: &'a Making,
    pub(super) bases: &'a [Branch],
    pub(super) taken: bool,
}

#[must_use]
pub(crate) fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

#[must_use]
pub(crate) fn named(worktree: &Worktree) -> String {
    match worktree.branch() {
        Some(branch) => branch.to_owned(),
        None => labels::file_name(worktree.path()),
    }
}

pub(super) fn picker(choosing: &Choosing<'_>) -> impl View {
    let mut labels: Vec<Label> = choosing
        .offers
        .iter()
        .map(|offer| Label::from(offer.label.clone()))
        .collect();
    if !choosing.locked {
        labels.push(tr!("ade.chat.worktree.make"));
    }

    let paths: Rc<[PathBuf]> = choosing
        .offers
        .iter()
        .map(|offer| offer.path.clone())
        .collect();
    combo(choosing.at)
        .options(labels)
        .placeholder(tr!("ade.chat.worktree.pick"))
        .control_size(ControlSize::Small)
        .enabled(!choosing.locked)
        .on_select(given_with_setup(
            move |project, setup, index: usize| match paths.get(index) {
                Some(path) => project.pick_worktree(path.clone(), setup),
                None => project.make_worktree(),
            },
        ))
        .width(PICKER)
}

pub(super) fn form(naming: &Naming<'_>) -> impl View {
    let making = naming.making;
    column! {
        row! {
            question_chip(&tr!("ade.chat.worktree.title"));
            spacer().grow(1.0);
        }
        .align_items(Align::Center)
        .fill_width();
        text_field(&making.branch)
            .placeholder(tr!("ade.chat.worktree.branch"))
            .autofocus(true)
            .control_size(ControlSize::Small)
            .on_change(act(|project| project.named_branch()))
            .on_submit(act(|project| project.create_worktree()))
            .fill_width();
        base(naming);
        if let Some(error_message) = making.error_message.as_deref() {
            notice(MaterialSymbol::ERROR, error_message, palette().danger);
        }
        onwards(making);
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn base(naming: &Naming<'_>) -> Element {
    if naming.taken {
        return row! {
            icon(MaterialSymbol::CALL_SPLIT)
                .extent(Length::px(16.0))
                .color(palette().text_muted);
            text(tr!("ade.chat.worktree.taken")).caption().muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .any();
    }

    let names: Rc<[String]> = naming
        .bases
        .iter()
        .map(|branch| branch.name().to_owned())
        .collect();
    let at = naming
        .making
        .base()
        .and_then(|base| names.iter().position(|name| name == base));

    combo(at)
        .options(naming.bases.iter().map(based))
        .placeholder(tr!("ade.chat.worktree.base"))
        .control_size(ControlSize::Small)
        .on_select(given(move |project, index: usize| match names.get(index) {
            Some(name) => {
                let name = name.clone();
                project.making(move |making| making.set_base(name));
            }
            None => skip_view(),
        }))
        .fill_width()
        .any()
}

fn based(branch: &Branch) -> String {
    match branch.kind() {
        Kind::Remote => tr!("ade.git.remote", branch = branch.name()).to_string(),
        Kind::Local => branch.name().to_owned(),
    }
}

fn onwards(making: &Making) -> impl View {
    row! {
        spacer().grow(1.0);
        button(text(tr!("ade.chat.worktree.cancel")).caption())
            .ghost()
            .control_size(ControlSize::Small)
            .on_press(act(|project| project.cancel_worktree()));
        button(text(tr!("ade.chat.worktree.create")).caption())
            .control_size(ControlSize::Small)
            .loading(making.busy)
            .enabled(!making.branch().is_empty() && !making.busy)
            .on_press(act(|project| project.create_worktree()));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
