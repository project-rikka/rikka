use rikka_app::prelude::*;
use settings_displays::Rotation;

pub(crate) fn label(rotation: Rotation) -> Label {
    match rotation {
        Rotation::None => tr!("settings.displays.rotation.none"),
        Rotation::Quarter => tr!("settings.displays.rotation.quarter"),
        Rotation::Half => tr!("settings.displays.rotation.half"),
        Rotation::ThreeQuarters => tr!("settings.displays.rotation.three-quarters"),
    }
}
