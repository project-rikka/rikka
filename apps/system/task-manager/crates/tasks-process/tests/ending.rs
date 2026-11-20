use std::collections::HashMap;
use std::process::Command;

use tasks_process::{Error, Process, Processes, kill, terminate};

fn sampled(pid: i32) -> Process {
    Processes::new()
        .sample(&HashMap::new())
        .iter()
        .flat_map(|group| group.processes())
        .find(|process| process.pid == pid)
        .cloned()
        .expect("child not sampled")
}

#[test]
fn terminate_spares_recycled_pid() {
    let mut child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("failed to spawn sleep");
    let pid = child.id() as i32;
    let process = sampled(pid);

    let mut recycled = process.clone();
    recycled.start_time += 1;
    assert!(matches!(terminate(&recycled), Err(Error::NoSuchProcess(missing)) if missing == pid));
    assert!(child.try_wait().expect("failed to poll child").is_none());

    kill(&process).expect("failed to kill child");
    assert!(!child.wait().expect("failed to reap child").success());
}
