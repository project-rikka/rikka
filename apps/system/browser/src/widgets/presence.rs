use rikka_app::prelude::*;

pub(crate) fn presence(shown: impl IntoIterator<Item = Element>) -> Element {
    column_of(shown).build()
}
