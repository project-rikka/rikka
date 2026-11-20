use std::sync::Arc;

use proton_session::{ErrorBanner, Failure};
use proton_vaults::{Change, HistoryEntry};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::pass::Pass;
use crate::{pane, wording};

#[derive(Debug, State)]
pub(crate) struct History {
    history_entries: Arc<Vec<HistoryEntry>>,
    busy: bool,
    error: ErrorBanner,
}

impl History {
    pub(crate) fn working() -> History {
        History {
            history_entries: Arc::new(Vec::new()),
            busy: true,
            error: ErrorBanner::default(),
        }
    }

    pub(crate) fn took(&mut self, history_entries: Vec<HistoryEntry>) {
        self.history_entries = Arc::new(history_entries);
        self.busy = false;
        self.error.clear();
    }

    pub(crate) fn failed(&mut self, failure: Failure) {
        self.busy = false;
        self.error.set(failure);
    }

    pub(crate) fn moment(&self, at: usize) -> Option<&HistoryEntry> {
        self.history_entries.get(at)
    }
}

pub(crate) fn view(history: &History) -> impl View {
    column! {
        pane::header(
            tr!("proton.pass.history"),
            |vault: &mut Pass| vault.close_history(),
            spacer(),
        );
        rule();
        pane::error_banner(&history.error, |history: &mut History| &mut history.error);
        body(history).grow(1.0).fill_width();
    }
    .fill()
}

fn body(history: &History) -> Element {
    if history.busy && history.history_entries.is_empty() {
        return pane::working(tr!("proton.pass.reading-history")).build();
    }
    if history.history_entries.is_empty() {
        return pane::nothing(MaterialSymbol::HISTORY, tr!("proton.pass.no-history")).build();
    }

    scrollable(
        column_of(
            history
                .history_entries
                .iter()
                .enumerate()
                .map(|(at, moment)| entry(at, moment).build()),
        )
        .gap(Space::NORMAL)
        .padding(Space::ROOMY)
        .fill_width(),
    )
    .grow(1.0)
    .fill_width()
    .build()
}

fn entry(at: usize, moment: &HistoryEntry) -> impl View {
    let newest = at == 0;

    card(
        column! {
            row! {
                icon(match moment.is_first() {
                    true => MaterialSymbol::BOLT,
                    false => MaterialSymbol::EDIT,
                })
                .extent(Length::px(16.0))
                .color(palette().accent);
                column! {
                    text(match moment.is_first() {
                        true => tr!("proton.pass.created"),
                        false => tr!("proton.pass.revision", count = &moment.revision.to_string()),
                    })
                    .label();
                    text(wording::whole(moment.at)).caption().muted();
                }
                .gap(Space::HAIR)
                .grow(1.0);
                if newest {
                    pane::badge(tr!("proton.pass.now"), palette().success);
                } else {
                    button(text(tr!("proton.pass.restore-revision")).caption())
                        .ghost()
                        .small()
                        .on_press(move |vault: &mut Pass| vault.restore_moment(at));
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            if !moment.changes.is_empty() && !moment.is_first() {
                row_of(moment.changes.iter().map(|change| {
                    pane::badge(named(*change), palette().accent).build()
                }))
                .gap(Space::TIGHT)
                .wrap_items()
                .fill_width();
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .fill_width()
}

fn named(change: Change) -> Label {
    match change {
        Change::Created => tr!("proton.pass.created"),
        Change::Title => tr!("proton.pass.title"),
        Change::Identifier => tr!("proton.pass.username"),
        Change::Password => tr!("proton.pass.password"),
        Change::Code => tr!("proton.pass.code"),
        Change::Websites => tr!("proton.pass.websites"),
        Change::Note => tr!("proton.pass.note"),
        Change::Fields => tr!("proton.pass.fields"),
        Change::Card => tr!("proton.pass.kind.card"),
        Change::Wifi => tr!("proton.pass.kind.wifi"),
        Change::Ssh => tr!("proton.pass.kind.ssh"),
    }
}
