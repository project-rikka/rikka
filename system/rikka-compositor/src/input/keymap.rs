use rikka_compositor_seat::{Keymap, KeymapNames, Repeat};
use rikka_ime::Keymap as Chosen;

use crate::error::{Error, Result};

pub(crate) const REPEAT: Repeat = Repeat {
    rate: 30,
    delay: 250,
};

fn names(chosen: Option<&Chosen>) -> KeymapNames {
    chosen.map_or_else(KeymapNames::default, |chosen| KeymapNames {
        rules: String::new(),
        model: chosen.model.clone(),
        layout: chosen.layout.clone(),
        variant: chosen.variant.clone(),
        options: chosen.options.clone(),
    })
}

pub(crate) fn compile(chosen: Option<&Chosen>) -> Result<Keymap> {
    let named = names(chosen);
    Keymap::from_names(&named)
        .or_else(|error| {
            tracing::warn!("cannot compile keymap {named:?}: {error}");
            Keymap::from_names(&KeymapNames::default())
        })
        .map_err(Error::Keymap)
}
