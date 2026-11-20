use rikka_image::Budget;

#[test]
fn reserve_blocks_until_budget_frees() {
    let budget = Budget::new(1024);
    let first = budget.reserve(768);

    let waiting = budget.clone();
    let worker = std::thread::spawn(move || drop(waiting.reserve(768)));

    assert_eq!(budget.reserved(), 768, "second reserve must wait");
    drop(first);
    worker.join().expect("waiting reserve proceeds");
    assert_eq!(budget.reserved(), 0);

    let oversized = budget.reserve(4096);
    assert_eq!(
        budget.reserved(),
        4096,
        "oversized reserve proceeds on empty budget"
    );
    drop(oversized);
}

#[test]
fn close_releases_waiting_reservations() {
    let budget = Budget::new(1024);
    let first = budget.reserve(1024);

    let waiting = budget.clone();
    let worker = std::thread::spawn(move || drop(waiting.reserve(1024)));

    budget.close();
    worker.join().expect("close releases waiting reserve");
    drop(first);
}
