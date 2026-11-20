use std::sync::OnceLock;

use rikka_renderer_3d::MaterialTemplate;

include!("presets.rs");

fn templates() -> &'static [(&'static str, MaterialTemplate)] {
    static TEMPLATES: OnceLock<Vec<(&'static str, MaterialTemplate)>> = OnceLock::new();
    TEMPLATES.get_or_init(|| {
        PRESETS
            .iter()
            .map(|(key, artifact)| (*key, MaterialTemplate::prebuilt(key, artifact)))
            .collect()
    })
}

pub fn template(key: &str) -> Option<MaterialTemplate> {
    templates()
        .iter()
        .find(|(prebuilt, _)| *prebuilt == key)
        .map(|(_, template)| template.clone())
}

pub fn keys() -> impl Iterator<Item = &'static str> {
    PRESETS.iter().map(|(key, _)| *key)
}
