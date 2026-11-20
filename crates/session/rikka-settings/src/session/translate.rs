use rikka_types::Choice;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.translate";

pub const TARGET: &str = "target";
pub const SOURCE: &str = "source";
pub const PROFILE: &str = "profile";
pub const BACKEND: &str = "model-backend";
pub const MODEL: &str = "model";
pub const EFFORT: &str = "effort";

pub const UNSET: &str = "";

#[must_use]
pub fn schema<Profile: Choice>(efforts: impl IntoIterator<Item = String>) -> Schema {
    Schema::new(DOMAIN)
        .summary("Translation window")
        .key(Key::string(TARGET, UNSET).summary(
            "The language to translate into, as a BCP-47 tag or as a language written out; \
             empty for this session's own language",
        ))
        .key(
            Key::string(SOURCE, UNSET).summary(
                "The language to translate from; empty to work it out from the text itself",
            ),
        )
        .key(Key::choice::<Profile>(PROFILE).summary(
            "How the translation is worded: natural keeps the original's tone; casual, polite \
             and business write it that way",
        ))
        .key(Key::string(BACKEND, UNSET).summary(
            "The account that translates, a backend id or an endpoint:<id>; empty for the one \
             the desktop answers with",
        ))
        .key(
            Key::string(MODEL, UNSET)
                .summary("The model that translates; empty for the account's own"),
        )
        .key(
            Key::string(EFFORT, UNSET)
                .summary(
                    "How hard the model thinks about a translation; empty for the desktop's own \
                     setting",
                )
                .choices(unset_or(efforts)),
        )
}

fn unset_or(choices: impl IntoIterator<Item = String>) -> Vec<String> {
    std::iter::once(UNSET.to_owned()).chain(choices).collect()
}
