use std::collections::HashMap;

use tasks_process::Processes;

#[test]
fn sample_lists_each_process_once() {
    let mut processes = Processes::new();

    let groups = processes.sample(&HashMap::new());

    let pids: Vec<i32> = groups
        .iter()
        .flat_map(|group| group.processes().map(|process| process.pid))
        .collect();
    let mut unique = pids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), pids.len(), "a process stands in one group");
    let own = std::process::id() as i32;
    let own_process = groups
        .iter()
        .flat_map(|group| group.processes())
        .find(|process| process.pid == own)
        .expect("this process is sampled");
    assert!(own_process.memory.is_some_and(|memory| memory > 0));
    assert!(own_process.threads >= 1);
}
