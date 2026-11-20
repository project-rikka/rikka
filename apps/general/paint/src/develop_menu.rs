use rikka_app::prelude::*;
use rikka_effects::Family;
use rikka_image_edit::Desaturate;

use crate::command::Command;
use crate::direct::{Direct, desaturation};
use crate::paint::Paint;
use crate::session::Kind;

const DESATURATIONS: [Desaturate; 3] = [
    Desaturate::Lightness,
    Desaturate::Luminosity,
    Desaturate::Average,
];

fn direct(direct: Direct, paint: &Paint) -> MenuItem {
    Command::Develop(direct).entry(direct.name(), paint)
}

fn dialog(kind: rikka_effects::Kind, paint: &Paint) -> MenuItem {
    let label = tr!("paint.menu.dialog", name = kind.name().as_str());
    Command::Begin(Kind::Develop(kind)).entry(label, paint)
}

pub(crate) fn adjustments(paint: &Paint) -> Vec<MenuItem> {
    let dialog = |kind: rikka_effects::Kind| dialog(kind, paint);
    let desaturations = DESATURATIONS.map(|method| {
        Command::Develop(Direct::Desaturate(method)).entry(desaturation(method), paint)
    });
    vec![
        direct(Direct::AutoLevels, paint),
        direct(Direct::AutoContrast, paint),
        direct(Direct::AutoColor, paint),
        direct(Direct::Equalize, paint),
        menu_separator(),
        dialog(rikka_effects::Kind::BrightnessContrast),
        dialog(rikka_effects::Kind::Levels),
        dialog(rikka_effects::Kind::Curves),
        dialog(rikka_effects::Kind::Exposure),
        menu_separator(),
        dialog(rikka_effects::Kind::Vibrance),
        dialog(rikka_effects::Kind::HueSaturation),
        dialog(rikka_effects::Kind::ColorBalance),
        dialog(rikka_effects::Kind::BlackWhite),
        dialog(rikka_effects::Kind::PhotoFilter),
        dialog(rikka_effects::Kind::ChannelMixer),
        dialog(rikka_effects::Kind::ColorLookup),
        dialog(rikka_effects::Kind::WhiteBalance),
        menu_separator(),
        direct(Direct::Invert, paint),
        dialog(rikka_effects::Kind::Posterize),
        dialog(rikka_effects::Kind::Threshold),
        dialog(rikka_effects::Kind::GradientMap),
        dialog(rikka_effects::Kind::SelectiveColor),
        menu_separator(),
        dialog(rikka_effects::Kind::ShadowsHighlights),
        menu_item(tr!("paint.operation.desaturate"))
            .enabled(paint.develops())
            .items(desaturations),
        direct(Direct::Sepia, paint),
    ]
}

pub(crate) fn effects(paint: &Paint) -> Vec<MenuItem> {
    let (repeat, reopen) = match paint.last_effect() {
        Some(last) => (
            tr!("paint.effects.repeat", name = last.name().as_str()),
            tr!("paint.menu.dialog", name = last.name().as_str()),
        ),
        None => (
            tr!("paint.effects.repeat_last"),
            tr!("paint.effects.reopen_last"),
        ),
    };
    let mut items = vec![
        Command::RepeatEffect.entry(repeat, paint),
        Command::ReopenEffect.entry(reopen, paint),
        menu_separator(),
    ];
    items.extend(
        Family::ALL
            .into_iter()
            .filter(|family| *family != Family::Adjust)
            .map(|family| {
                let mut members: Vec<MenuItem> =
                    family.kinds().map(|kind| dialog(kind, paint)).collect();
                if family == Family::Stylize {
                    members.extend([menu_separator(), direct(Direct::FindEdges, paint)]);
                }
                menu_item(family.name()).items(members)
            }),
    );
    items
}
