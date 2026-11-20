use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::project::git::operation::GitOperation;
use crate::project::git::state::Git;
use crate::project::git::transfer::Transfer;
use crate::project::git::transferring::Transferring;
use crate::project::git::{SMALL, at_git};

const GLYPH: f32 = 16.0;

enum Primary {
    Commit,
    Offer(Transfer),
    Running(Transferring),
}

impl Primary {
    fn of(git: &Git) -> Primary {
        let committing = git.has_ticked();
        if let Some(transferring) = git.transferring()
            && (transferring.transfer().is_exclusive() || !committing)
        {
            return Primary::Running(transferring);
        }
        if committing || git.remote().is_none() {
            return Primary::Commit;
        }
        Primary::Offer(git.transfer())
    }
}

pub(super) fn view(git: &Git) -> Element {
    match Primary::of(git) {
        Primary::Commit => commit(git).any(),
        Primary::Offer(transfer) => offer(git, transfer).any(),
        Primary::Running(transferring) => running(git, transferring).any(),
    }
}

fn commit(git: &Git) -> impl View {
    let label = match git.status().branch() {
        Some(branch) => tr!("ade.git.commit-to", branch = branch),
        None => tr!("ade.git.commit"),
    };
    button(
        text(label)
            .caption()
            .medium()
            .no_wrap()
            .align(TextAlign::Center)
            .fill_width(),
    )
    .small()
    .loading(git.operation() == Some(GitOperation::Committing))
    .enabled(git.is_committable())
    .on_press(at_git(Git::commit))
    .fill_width()
}

fn offer(git: &Git, transfer: Transfer) -> impl View {
    let status = git.status();
    let (ahead, behind) = (status.ahead(), status.behind());
    let remote = git.remote().unwrap_or_default();
    let label = match transfer {
        Transfer::Fetch => tr!("ade.git.sync.fetch", remote = remote),
        Transfer::Pull => tr!("ade.git.sync.pull", remote = remote),
        Transfer::Push => tr!("ade.git.sync.push", remote = remote),
        Transfer::Publish => tr!("ade.git.sync.publish"),
    };
    button(
        row! {
            icon(glyph(transfer)).extent(Length::px(GLYPH));
            text(label).caption().medium().no_wrap();
            if behind > 0 {
                count(MaterialSymbol::ARROW_DOWNWARD, behind);
            }
            if ahead > 0 {
                count(MaterialSymbol::ARROW_UPWARD, ahead);
            }
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .small()
    .enabled(git.is_transferable())
    .on_press(at_git(Git::sync))
    .fill_width()
}

fn running(git: &Git, transferring: Transferring) -> impl View {
    let remote = git.remote().unwrap_or_default();
    let label = match transferring.transfer() {
        Transfer::Fetch => tr!("ade.git.sync.fetching", remote = remote),
        Transfer::Pull => tr!("ade.git.sync.pulling", remote = remote),
        Transfer::Push => tr!("ade.git.sync.pushing", remote = remote),
        Transfer::Publish => tr!("ade.git.sync.publishing"),
    };
    button(
        row! {
            text(label).caption().medium().no_wrap();
            if let Some(progress) = transferring.progress() {
                text(tr!("ade.git.sync.percent", percent = progress.percent()))
                    .caption()
                    .no_wrap();
            }
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .small()
    .loading(true)
    .fill_width()
}

fn count(glyph: MaterialSymbol, count: usize) -> impl View {
    row! {
        icon(glyph).extent(Length::px(SMALL));
        text(count.to_string()).caption().no_wrap();
    }
    .gap(Space::HAIR)
    .align_items(Align::Center)
}

fn glyph(transfer: Transfer) -> MaterialSymbol {
    match transfer {
        Transfer::Fetch => MaterialSymbol::SYNC,
        Transfer::Pull => MaterialSymbol::ARROW_DOWNWARD,
        Transfer::Push => MaterialSymbol::ARROW_UPWARD,
        Transfer::Publish => MaterialSymbol::CLOUD_UPLOAD,
    }
}
