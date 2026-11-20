use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

use rikka_sync::{Condvar, Mutex, RwLock};

fn panic_inside(body: impl FnOnce()) {
    let outcome = panic::catch_unwind(AssertUnwindSafe(body));
    assert!(outcome.is_err());
}

#[test]
fn mutex_stays_usable_after_holder_panics() {
    let mut mutex = Mutex::new(1);
    panic_inside(|| {
        let mut guard = mutex.lock();
        *guard = 2;
        panic!("holder panicked");
    });
    assert_eq!(*mutex.lock(), 2);
    assert_eq!(mutex.try_lock().map(|guard| *guard), Some(2));
    *mutex.get_mut() = 3;
    assert_eq!(mutex.into_inner(), 3);
}

#[test]
fn rw_lock_stays_usable_after_writer_panics() {
    let mut lock = RwLock::new(1);
    panic_inside(|| {
        let mut guard = lock.write();
        *guard = 2;
        panic!("writer panicked");
    });
    assert_eq!(*lock.read(), 2);
    *lock.write() = 3;
    assert_eq!(lock.try_read().map(|guard| *guard), Some(3));
    *lock.get_mut() = 4;
    assert_eq!(lock.into_inner(), 4);
}

#[test]
fn condvar_waits_on_poisoned_mutex() {
    let mutex = Mutex::new(false);
    let condvar = Condvar::new();
    panic_inside(|| {
        let _guard = mutex.lock();
        panic!("holder panicked");
    });
    std::thread::scope(|scope| {
        scope.spawn(|| {
            *mutex.lock() = true;
            condvar.notify_all();
        });
        let guard = condvar.wait_while(mutex.lock(), |ready| !*ready);
        assert!(*guard);
    });
    let (guard, timeout) =
        condvar.wait_timeout_while(mutex.lock(), Duration::ZERO, |ready| !*ready);
    assert!(*guard && !timeout.timed_out());
}

#[test]
fn try_lock_reports_held_lock() {
    let mutex = Mutex::new(());
    let _guard = mutex.lock();
    assert!(mutex.try_lock().is_none());
}
