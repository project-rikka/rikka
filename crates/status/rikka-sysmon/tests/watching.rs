use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use rikka_sysmon::Machine;

#[test]
fn watched_machine_reports_samples() {
    let machine = Machine::with_interval(Duration::from_millis(50));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime should build");

    let vitals = runtime.block_on(async {
        let mut changes = std::pin::pin!(machine.changes());
        tokio::time::timeout(Duration::from_secs(10), changes.next())
            .await
            .expect("reading should arrive")
            .expect("machine should keep sampling")
    });

    assert!(vitals.memory.total > 0, "this machine has memory");
    assert!(vitals.topology.logical > 0, "and a processor");
    assert!(Arc::ptr_eq(&vitals, &machine.state()));
}

#[test]
fn unwatched_machine_does_not_sample() {
    let machine = Machine::with_interval(Duration::from_millis(10));

    std::thread::sleep(Duration::from_millis(100));

    assert_eq!(machine.state().memory.total, 0);
}
