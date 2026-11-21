use installer_core::{Catalog, Error, Offer};

use crate::support::catalog;

#[test]
fn catalog_lists_required_first() {
    let catalog = catalog();
    let offers: Vec<(Offer, &str)> = catalog
        .components()
        .iter()
        .map(|component| (component.offer, component.id.as_str()))
        .collect();
    assert_eq!(
        offers,
        [
            (Offer::Required, "files"),
            (Offer::Required, "base"),
            (Offer::Selected, "browser"),
            (Offer::Selected, "paint"),
            (Offer::Offered, "cut"),
        ]
    );
}

#[test]
fn malformed_catalog_is_refused() {
    for json in ["", "[]", r#"{"base": {"name": "Rikka"}}"#] {
        assert!(
            matches!(Catalog::parse(json), Err(Error::Catalog(_))),
            "{json}"
        );
    }
}
