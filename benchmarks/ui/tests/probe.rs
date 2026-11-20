use std::os::fd::BorrowedFd;

use ui_scene::{Probe, Recording, monotonic_ns};

#[test]
fn probe_frames_reach_the_recording() {
    let recording = Recording::new().expect("memfd is available");
    let fd = rustix::io::dup(unsafe { BorrowedFd::borrow_raw(recording.child_fd()) })
        .expect("the recording fd duplicates");
    let started = monotonic_ns();
    let probe = Probe::from_fd(fd);
    for _ in 0..5 {
        probe.frame();
    }
    assert_eq!(recording.frames().expect("the frame count reads"), 5);
    let stamps = recording.stamps().expect("the recording reads");
    assert_eq!(stamps.frames, 5);
    assert_eq!(stamps.between(started, monotonic_ns()).len(), 5);
    assert!(stamps.times.windows(2).all(|pair| pair[0] <= pair[1]));
}
