use std::sync::LazyLock;

use rikka_settings::Catalog;

static CATALOG: LazyLock<Result<Catalog, String>> = LazyLock::new(|| {
    Catalog::from_json(include_str!(env!("RIKKA_SETTINGS_CATALOG")))
        .map_err(|error| error.to_string())
});

pub(crate) fn catalog() -> Result<&'static Catalog, String> {
    CATALOG.as_ref().map_err(Clone::clone)
}
