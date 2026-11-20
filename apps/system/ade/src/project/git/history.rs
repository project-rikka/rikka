use std::sync::Arc;

use ade_git::Commit;
use rikka_app::prelude::*;
use rikka_chat::{CARD_INSET, CODE_FONT_SIZE, mono};
use rikka_widgets::{column, row};

use crate::hint::hint;
use crate::labels;
use crate::project::git::at_git_given;
use crate::project::git::state::Git;
use crate::project::git::subject::Subject;

const SECOND: u64 = 1_000;

pub(super) fn commits(git: &Git) -> Element {
    let history = git.history();
    if history.is_empty() {
        return hint(match git.is_recalling() {
            true => tr!("ade.git.recalling"),
            false => tr!("ade.git.uncommitted"),
        })
        .any();
    }

    let open = match git.shown() {
        Some((Subject::Commit(oid), _)) => Some(oid.as_str()),
        _ => None,
    };
    let selected = open.and_then(|open| history.iter().position(|commit| commit.oid() == open));
    let chosen: Arc<[Commit]> = Arc::clone(history);

    list(history.iter().map(row_for))
        .selected(selected)
        .control_size(ControlSize::Small)
        .on_select(at_git_given(move |git, index: usize| {
            match chosen.get(index) {
                Some(commit) => git.show(Subject::Commit(commit.oid().to_owned())),
                None => skip_view(),
            }
        }))
        .fill_width()
        .any()
}

fn row_for(commit: &Commit) -> ListItem {
    list_item(summary(commit))
        .keyed(commit.oid())
        .secondary(format!(
            "{} · {}",
            commit.author(),
            labels::time_ago(commit.at().saturating_mul(SECOND))
        ))
        .icon(MaterialSymbol::COMMIT)
}

fn summary(commit: &Commit) -> String {
    match commit.summary().trim() {
        "" => tr!("ade.git.unsaid").to_string(),
        other => other.to_owned(),
    }
}

pub(super) fn heading(commit: &Commit) -> impl View {
    let palette = palette();
    column! {
        text(summary(commit))
            .label()
            .medium()
            .color(palette.text)
            .fill_width();
        if !commit.body().is_empty() {
            text(commit.body().to_owned()).caption().muted().fill_width();
        }
        row! {
            icon(MaterialSymbol::PERSON)
                .extent(Length::px(14.0))
                .color(palette.text_muted);
            text(commit.author()).caption().muted().no_wrap();
            text(labels::time_ago(commit.at().saturating_mul(SECOND)))
                .caption()
                .muted()
                .no_wrap();
            spacer().grow(1.0);
            mono(commit.short(), CODE_FONT_SIZE).muted().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
    .padding(Inset::xy(CARD_INSET, 10.0))
}
