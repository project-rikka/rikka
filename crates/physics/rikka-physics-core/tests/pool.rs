use rikka_physics_core::Pool;

rikka_physics_core::id!(ThingId);

#[test]
fn stale_ids_miss_after_slot_reuse() {
    let mut pool = Pool::<ThingId, &str>::new();
    let a = pool.insert("a");
    let b = pool.insert("b");
    assert_eq!(pool.remove(a), Some("a"));
    assert_eq!(pool.remove(a), None);
    let c = pool.insert("c");
    assert_eq!(c.index, a.index);
    assert_ne!(c, a);
    assert_eq!(pool.get(a), None);
    assert_eq!(pool[c], "c");
    assert_eq!(pool[b], "b");
    assert_eq!(pool.len(), 2);
}

#[test]
fn reset_pool_matches_fresh_pool() {
    let mut pool = Pool::<ThingId, u32>::new();
    let fresh: Vec<_> = (0..3).map(|value| pool.insert(value)).collect();
    pool.remove(fresh[1]);
    pool.reset();
    assert!(pool.is_empty());
    let again: Vec<_> = (0..3).map(|value| pool.insert(value)).collect();
    assert_eq!(again, fresh);
}
