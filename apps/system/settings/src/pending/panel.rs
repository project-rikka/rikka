use rikka_app::prelude::*;
use rikka_nix::Stage;
use rikka_widgets::row;

use crate::notice::{acting_notice, notice};
use crate::pending::component::Pending;

const SPINNER: f32 = 14.0;

pub(crate) fn panel(pending: &Pending, apply: Handler, discard: Handler) -> Option<Element> {
    if let Some(reason) = pending.unavailable() {
        return Some(
            notice(
                MaterialSymbol::CLOUD_OFF,
                palette().warning,
                tr!("settings.pending.unavailable", reason = reason),
            )
            .build(),
        );
    }

    let refusal = pending.refusal().map(|refusal| {
        acting_notice(
            MaterialSymbol::ERROR,
            palette().danger,
            tr!(
                "settings.pending.not-started",
                reason = &refusal.to_string()
            ),
            tr!("settings.pending.close"),
            |pending: &mut Pending| pending.dismiss_refusal(),
        )
    });
    let overridden = (!pending.overridden().is_empty()).then(|| {
        let files: Vec<&str> = pending
            .overridden()
            .iter()
            .map(|module| module.file_name())
            .collect();
        acting_notice(
            MaterialSymbol::WARNING,
            palette().warning,
            tr!("settings.pending.overridden", files = &files.join(", ")),
            tr!("settings.pending.close"),
            |pending: &mut Pending| pending.dismiss_overridden(),
        )
    });
    let waiting = match (pending.stage(), pending.changed()) {
        (Stage::Idle, true) => Some(waiting(apply, discard, None)),
        (Stage::Idle, false) => None,
        (stage, changed) => {
            let label = match stage {
                Stage::Requested => tr!("settings.pending.asking"),
                Stage::Idle | Stage::Running => tr!("settings.pending.applying"),
            };
            Some(match changed {
                true => waiting(apply, discard, Some(label)),
                false => working(label),
            })
        }
    };
    let edit_error = pending.error().map(|reason| {
        notice(
            MaterialSymbol::ERROR,
            palette().danger,
            tr!("settings.pending.refused", reason = reason),
        )
    });
    let shown: Vec<Element> = [
        refusal.map(View::build),
        overridden.map(View::build),
        edit_error.map(View::build),
        waiting.map(|waiting| card(waiting).build()),
    ]
    .into_iter()
    .flatten()
    .collect();
    if shown.is_empty() {
        return None;
    }
    Some(column_of(shown).gap(Space::SNUG).fill_width().build())
}

fn working(label: Label) -> Flex {
    row! {
        spinner().extent(Length::px(SPINNER));
        text(label).label().grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn waiting(apply: Handler, discard: Handler, working: Option<Label>) -> Flex {
    let busy = working.is_some();
    let applying = match working {
        Some(working) => button(
            row! {
                spinner().extent(Length::px(SPINNER));
                text(working);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .primary()
        .enabled(false),
        None => button(tr!("settings.pending.apply"))
            .primary()
            .on_press(apply),
    };
    row! {
        icon(MaterialSymbol::EDIT).extent(Length::px(18.0));
        text(tr!("settings.pending.waiting")).label().grow(1.0);
        button(tr!("settings.pending.discard"))
            .ghost()
            .enabled(!busy)
            .on_press(discard);
        applying;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
