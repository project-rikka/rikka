use rikka_physics_tailor::Quality;
use rikka_settings::{Key, Schema};

use super::Preferences;

const QUALITY: &str = "cloth-quality";
const QUALITIES: [(Quality, &str); 3] = [
    (Quality::Low, "low"),
    (Quality::Medium, "medium"),
    (Quality::High, "high"),
];

impl Preferences {
    pub(crate) fn cloth_quality(&self) -> Quality {
        let stored = self.0.borrow().get::<String>(QUALITY);
        QUALITIES
            .iter()
            .find(|(_, name)| *name == stored)
            .map_or(Quality::High, |(quality, _)| *quality)
    }

    pub(crate) fn keep_cloth_quality(&self, quality: Quality) {
        let name = QUALITIES
            .iter()
            .find(|(candidate, _)| *candidate == quality)
            .map_or("high", |(_, name)| *name);
        self.0.borrow_mut().set(QUALITY, name);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema.key(
        Key::string(QUALITY, "high")
            .choices(QUALITIES.iter().map(|(_, name)| (*name).to_owned()))
            .summary("Detail of the simulated cloth and hair"),
    )
}
