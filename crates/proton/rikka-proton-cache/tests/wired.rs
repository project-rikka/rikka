use rikka_proton_cache::{Cache, Table, Wired};
use rikka_proton_pgp::SymmetricKey;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Deserialize)]
struct Folded {
    #[serde(rename = "ID")]
    id: String,
}

const FOLDED: Table<Wired<Folded>> = Table::new("folded");

#[test]
fn wire_objects_keep_every_field() {
    let wire = json!({"ID": "link", "Unknown": {"Nested": [1, 2]}});
    let wired: Wired<Folded> = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(wired.id, "link");

    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    cache.put(&FOLDED, "link", wired);
    cache.flush().unwrap();

    let cold = cache.entries(&FOLDED);
    assert_eq!(cold.len(), 1);
    assert_eq!(cold[0].1.wire(), &wire);
    assert_eq!(cold[0].1.value().id, "link");
}
