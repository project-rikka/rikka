use std::sync::OnceLock;

use crate::catalog::config::CatalogConfig;
use crate::catalog::state::Catalog;

pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::load(&CatalogConfig::from_env()))
}
