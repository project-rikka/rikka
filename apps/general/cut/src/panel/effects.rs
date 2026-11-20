use cut_project::{Channel, Clip, ClipId, EffectSlot, Property};
use rikka_app::prelude::*;
use rikka_effects::{Effect, Family};
use rikka_effects_forms::Tuning;
use rikka_widgets::{column, row};

use super::keys::Keying;
use crate::controls::glyph_button;
use crate::cut::{Cut, Popup};

const ADD: &str = "cut-add-effect";
const GLYPH: f32 = 16.0;
const MENU: f32 = 220.0;
const OFF: f32 = 0.5;

pub(crate) fn effects(cut: &Cut, clip: &Clip, keying: Keying, frame: i64) -> impl View {
    let previewed = cut.live().effects(clip.id());
    let stack = previewed.as_deref().unwrap_or(clip.effects());
    let local = clip.local(frame) as f64;
    let mut rows = vec![
        row! {
            text(tr!("cut.panel.effects")).label().semibold().grow(1.0);
            glyph_button(MaterialSymbol::ADD, GLYPH)
                .ghost()
                .on_press(|cut: &mut Cut| cut.open_popup(Popup::Effects))
                .tooltip(tr!("cut.panel.add_effect"))
                .keyed(ADD);
        }
        .align_items(Align::Center)
        .fill_width()
        .build(),
    ];
    rows.extend(
        stack
            .iter()
            .enumerate()
            .map(|(at, slot)| card(clip.id(), at, stack.len(), slot, local, keying)),
    );
    column_of(rows).gap(Space::SNUG).fill_width()
}

fn card(
    clip: ClipId,
    at: usize,
    count: usize,
    slot: &EffectSlot,
    local: f64,
    keying: Keying,
) -> Element {
    let kind = slot.effect.kind();
    let enabled = slot.enabled;
    let effect = slot.effect_at(local);
    let tracks = slot.tracks.clone();
    let tuning = Tuning::new(
        move |cut: &mut Cut, effect: Effect| cut.preview_effect(at, effect),
        move |cut: &mut Cut, effect: Effect| cut.retune_effect(at, effect),
    )
    .adorn(move |knob: &'static str| {
        Some(
            keying.diamond(
                Property::Effect {
                    slot: at,
                    knob: knob.to_owned(),
                },
                tracks
                    .get(knob)
                    .filter(|track| !track.is_empty())
                    .map(Channel::Scalar),
            ),
        )
    });
    let palette = palette();
    let ink = match enabled {
        true => palette.text,
        false => palette.text.scale_alpha(OFF),
    };
    local_state(true, move |open: bool, set| {
        let folding = container(
            row! {
                icon(match open {
                    true => MaterialSymbol::EXPAND_MORE,
                    false => MaterialSymbol::CHEVRON_RIGHT,
                })
                .extent(Length::px(GLYPH))
                .color(palette.text_muted);
                text(kind.name()).color(ink).no_wrap().grow(1.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width(),
        )
        .on_press(move || set(!open))
        .focusless()
        .grow(1.0);
        let header = row! {
            checkbox(enabled)
                .control_size(ControlSize::Small)
                .on_toggle(move |cut: &mut Cut, on: bool| cut.enable_effect(at, on));
            folding;
            glyph_button(MaterialSymbol::ARROW_UPWARD, GLYPH)
                .ghost()
                .enabled(at > 0)
                .on_press(move |cut: &mut Cut| cut.move_effect(at, false))
                .tooltip(tr!("cut.panel.move_up"));
            glyph_button(MaterialSymbol::ARROW_DOWNWARD, GLYPH)
                .ghost()
                .enabled(at + 1 < count)
                .on_press(move |cut: &mut Cut| cut.move_effect(at, true))
                .tooltip(tr!("cut.panel.move_down"));
            glyph_button(MaterialSymbol::CLOSE, GLYPH)
                .ghost()
                .on_press(move |cut: &mut Cut| cut.remove_effect(at))
                .tooltip(tr!("cut.panel.remove_effect"));
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width();
        container(
            column! {
                header;
                if open {
                    rikka_effects_forms::form(&effect, &tuning);
                }
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .background(palette.surface_variant)
        .radius(Corner::Item)
        .padding(Space::SNUG)
        .fill_width()
    })
    .keyed(("cut-effect", clip, at, kind))
}

pub(crate) fn adding() -> Element {
    let families =
        Family::ALL.map(|family| {
            menu_item(family.name()).items(family.kinds().map(|kind| {
                menu_item(kind.name()).on_press(move |cut: &mut Cut| cut.add_effect(kind))
            }))
        });
    menu()
        .items(families)
        .width(MENU)
        .on_activate(|cut: &mut Cut, _: usize| cut.close_popup())
        .on_dismiss(Cut::close_popup)
        .build()
        .keyed("cut-effect-menu")
        .anchored(Anchor::new(ADD, AnchorEdge::BelowEnd))
}
