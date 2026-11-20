use rikka_proton_cache::{Cache, Table};
use rikka_proton_pgp::SymmetricKey;

const LISTS: Table<Vec<u32>> = Table::new("lists");

#[test]
fn newest_staged_write_wins() {
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let slot = cache.slot(&LISTS, "inbox");

    let older = slot.stage(vec![1]);
    let newer = slot.stage(vec![1, 2]);
    newer.write();
    older.write();
    cache.flush().unwrap();

    assert_eq!(slot.read(), Some(vec![1, 2]));

    slot.forget();
    assert_eq!(slot.read(), None);
}
