use animate_hold::{Holder, MAX_EASE};
use animate_ui::labelled;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::{Desk, HoldDial, Listed};

const GLYPH: f32 = 16.0;
const REACH: (f32, f32) = (0.01, 0.25);
const EASE: (f32, f32) = (0.0, MAX_EASE as f32);

pub(super) fn holds(desk: Desk<'_>, listed: &[Listed]) -> impl View {
    column_of(listed.iter().map(|entry| hold(desk, entry).any()))
        .gap(Space::SNUG)
        .fill_width()
}

fn hold(desk: Desk<'_>, listed: &Listed) -> impl View {
    let hold = listed.id;
    let (symbol, holder) = match listed.hold.holder {
        Holder::Hand(side) => (
            MaterialSymbol::BACK_HAND,
            animate_ui::holding_hand_label(side),
        ),
        Holder::Pin { .. } => (MaterialSymbol::PUSH_PIN, tr!("animate.hold.pin")),
    };
    let action = match listed.holding {
        true => button(text(tr!("animate.hold.release")).caption().no_wrap())
            .small()
            .secondary()
            .focusless()
            .on_press(desk.act(move |studio| {
                if let Some(id) = studio.subject_figure() {
                    studio.release_hold(id, hold);
                }
            }))
            .any(),
        false => button(icon(MaterialSymbol::DELETE).extent(Length::px(GLYPH)))
            .icon_only()
            .small()
            .ghost()
            .focusless()
            .on_press(desk.act(move |studio| {
                if let Some(id) = studio.subject_figure() {
                    studio.drop_hold(id, hold);
                }
            }))
            .tooltip(tr!("animate.tip.delete_hold")),
    };
    column! {
        row_of([
            icon(symbol).extent(Length::px(GLYPH)).any(),
            text(holder).no_wrap().any(),
            text(listed.owner.clone())
                .caption()
                .color(palette().text_muted)
                .no_wrap()
                .grow(1.0)
                .any(),
            action.any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width();
        dial(desk, listed, HoldDial::Reach, tr!("animate.hold.reach"), REACH);
        dial(desk, listed, HoldDial::Ease, tr!("animate.hold.ease"), EASE);
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn dial(
    desk: Desk<'_>,
    listed: &Listed,
    dial: HoldDial,
    label: Label,
    (least, most): (f32, f32),
) -> impl View {
    let hold = listed.id;
    let control = slider(desk.hold_dial(listed, dial).clamp(least, most)).range(least..=most);
    let control = match dial {
        HoldDial::Ease => control.step(1.0),
        HoldDial::Reach => control,
    };
    labelled(
        label,
        control
            .control_size(ControlSize::Small)
            .on_change(desk.act_with(move |studio, value: f32| {
                studio.draft_hold(hold, dial, value);
            }))
            .on_commit(desk.act_with(move |studio, value: f32| {
                studio.apply_hold(hold, dial, value);
            }))
            .fill_width(),
        None,
    )
}
