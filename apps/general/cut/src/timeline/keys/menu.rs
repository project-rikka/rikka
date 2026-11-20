use cut_project::Interpolation;
use rikka_app::prelude::*;
use rikka_keyframe::{Ease, Easing, Family};
use rikka_timeline::{ease_label, family_label};

use crate::command::Command;
use crate::cut::{Cut, Popup};
use crate::wording;

const PIN: &str = "cut-key-pin";
const WIDTH: f32 = 240.0;

fn shared(cut: &Cut) -> Option<Interpolation> {
    let clip = cut.selected_animated()?;
    let picked = cut.picked_keys();
    let mut interpolations = picked.iter().map(|key| clip.interpolation(key));
    let first = interpolations.next()??;
    interpolations
        .all(|other| other == Some(first))
        .then_some(first)
}

fn anchor() -> Anchor {
    Anchor::new(PIN, AnchorEdge::BelowStart)
}

pub(crate) fn pin(cut: &Cut) -> Option<Element> {
    let at = cut.keyframing().pin()?;
    let picking = matches!(cut.popup(), Some(Popup::Easing(picking)) if picking.anchor == anchor());
    (cut.keyframing().menu().is_some() || picking).then(|| {
        container(spacer())
            .size(0.0, 0.0)
            .position(at.x, at.y)
            .build()
            .keyed(PIN)
    })
}

pub(crate) fn context_menu(cut: &Cut) -> Option<Element> {
    cut.keyframing().menu()?;
    if cut.pointing().is_some() {
        return None;
    }
    let current = shared(cut);
    let entry = |interpolation: Interpolation, label: Label| {
        Command::Interpolate(interpolation)
            .entry_named(label, cut)
            .selected(current == Some(interpolation))
    };
    let named =
        |interpolation: Interpolation| entry(interpolation, wording::interpolation(interpolation));
    let curves = Family::ALL
        .into_iter()
        .filter(|curve| *curve != Family::Linear)
        .map(|curve| {
            menu_item(family_label(curve)).items(Ease::ALL.map(|ease| {
                entry(
                    Interpolation::Preset(Easing::new(curve, ease)),
                    ease_label(ease),
                )
            }))
        });
    let mut items = vec![menu_header(tr!("cut.keys.interpolation"))];
    items.extend(Interpolation::PLAIN.map(named));
    items.push(menu_separator());
    items.extend(Interpolation::EASY.map(named));
    items.push(menu_separator());
    items.push(menu_item(tr!("cut.keys.curves")).items(curves));
    items.push(
        menu_item(tr!("cut.keys.edit_curve"))
            .enabled(!cut.picked_keys().is_empty())
            .on_press(|cut: &mut Cut| cut.edit_key_curve(anchor())),
    );
    items.push(menu_separator());
    items.push(Command::Copy.entry(cut));
    items.push(Command::Delete.entry(cut));
    Some(
        menu()
            .items(items)
            .width(WIDTH)
            .on_activate(|cut: &mut Cut, _: usize| cut.close_key_menu())
            .on_dismiss(Cut::close_key_menu)
            .build()
            .keyed("cut-key-menu")
            .anchored(anchor()),
    )
}
