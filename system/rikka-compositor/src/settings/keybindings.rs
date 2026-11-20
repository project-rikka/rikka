use rikka_input::{ActionMap, Chord};
use rikka_session::profile::Profile;
use rikka_session::shortcuts::{DOMAIN, Shortcut};
use rikka_settings::seam::Settings;
use rikka_settings::{Key, Schema, Value};

use crate::input::KeyAction;

pub(crate) fn schema() -> Schema {
    Shortcut::all().fold(
        Schema::new(DOMAIN).summary("Compositor keybindings"),
        |schema, shortcut| {
            let shortcut = shortcut.describe();
            schema.key(
                Key::string(shortcut.name, shortcut.chord)
                    .summary(shortcut.summary)
                    .checked_with(validate_chord),
            )
        },
    )
}

fn validate_chord(value: &Value) -> Result<(), String> {
    let Value::String(text) = value else {
        return Err("expected a key combination".to_owned());
    };
    chord(text).map(drop)
}

pub(crate) fn action_map(settings: &Settings, profile: Profile) -> ActionMap<KeyAction> {
    Shortcut::all()
        .filter(|shortcut| shortcut.is_available_in(profile))
        .fold(ActionMap::new(), |map, shortcut| {
            let name = shortcut.describe().name;
            match bound(settings, &name) {
                Ok(Some(chord)) => {
                    tracing::debug!("{DOMAIN}/{name} = {chord}");
                    map.bind(chord, KeyAction::Run(shortcut))
                }
                Ok(None) => {
                    tracing::debug!("{DOMAIN}/{name} is unbound");
                    map
                }
                Err(reason) => {
                    tracing::warn!("cannot bind `{DOMAIN}/{name}`: {reason}");
                    map
                }
            }
        })
}

fn bound(settings: &Settings, name: &str) -> Result<Option<Chord>, String> {
    let text = settings
        .get::<String>(name)
        .map_err(|error| error.to_string())?;
    chord(&text)
}

fn chord(text: &str) -> Result<Option<Chord>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    text.parse()
        .map(Some)
        .map_err(|error: rikka_input::Error| error.to_string())
}
