use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use rikka_open::spawn_detached;

fn soon(mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn detached_returns_pid_and_reaps() {
    let folder = std::env::temp_dir().join(format!("rikka-open-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let started: Vec<_> = (0..3)
        .map(|index| {
            let written = folder.join(index.to_string());
            let pid = spawn_detached(
                Command::new("sh")
                    .arg("-c")
                    .arg("echo $$ > \"$1\"")
                    .arg("sh")
                    .arg(&written),
            )
            .unwrap();
            (written, pid)
        })
        .collect();

    for (written, pid) in &started {
        assert!(soon(
            || std::fs::read_to_string(written).is_ok_and(|contents| contents.ends_with('\n'))
        ));
        assert_eq!(
            std::fs::read_to_string(written).unwrap().trim(),
            pid.to_string()
        );
        let alive = format!("/proc/{pid}");
        assert!(
            soon(|| !Path::new(&alive).exists()),
            "{alive} is left behind"
        );
    }
    std::fs::remove_dir_all(&folder).unwrap();
}
