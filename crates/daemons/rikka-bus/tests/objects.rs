use std::sync::mpsc::{self, Receiver};

use rikka_bus::{ManagedObjects, ObjectChanges, Watch};
use rikka_test_bus::{PrivateBus, TIMEOUT};
use rikka_test_objects::FakeService;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

const SERVICE: &str = "org.rikka.Objects";
const ROOT: &str = "/org/rikka";
const THING: &str = "org.rikka.Thing";
const FIRST: &str = "/org/rikka/first";
const SECOND: &str = "/org/rikka/second";
const THIRD: &str = "/org/rikka/third";

struct Following {
    connection: Connection,
    objects: ManagedObjects,
    changes: Receiver<ObjectChanges>,
    _watch: Watch,
}

impl Following {
    fn start() -> Following {
        let connection = Connection::session().expect("the private bus should accept a client");
        let (heard, changes) = mpsc::channel();
        let watch = ManagedObjects::watch(&connection, SERVICE, ROOT, move |changes| {
            heard.send(changes).is_ok()
        })
        .expect("the bus should accept the watch");
        let objects =
            ManagedObjects::read(&connection, SERVICE, ROOT).expect("the service should answer");
        Following {
            connection,
            objects,
            changes,
            _watch: watch,
        }
    }

    fn next(&mut self) -> bool {
        let changes = self
            .changes
            .recv_timeout(TIMEOUT)
            .expect("the service should signal");
        self.objects
            .follow(&self.connection, changes)
            .expect("the changes should apply")
    }

    fn until(&mut self, settled: impl Fn(&ManagedObjects) -> bool) {
        while !settled(&self.objects) {
            self.next();
        }
    }

    fn level(&self, path: &str) -> Option<u32> {
        self.objects.get(path, THING)?.uint32("Level")
    }
}

fn thing(service: &FakeService, path: &str, level: u32) {
    service.add(path, THING, [("Level", Value::from(level))]);
}

#[test]
fn tree_follows_added_changed_and_removed_objects() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };
    let service = FakeService::serve(SERVICE, ROOT);
    thing(&service, FIRST, 1);
    thing(&service, THIRD, 3);
    let mut following = Following::start();
    assert_eq!(following.level(FIRST), Some(1));
    assert_eq!(following.level(THIRD), Some(3));

    service.set(FIRST, THING, "Level", Value::from(5_u32));
    thing(&service, SECOND, 2);
    service.remove(THIRD, THING);
    following.until(|objects| objects.get(THIRD, THING).is_none());

    assert_eq!(following.level(FIRST), Some(5));
    assert_eq!(following.level(SECOND), Some(2));
    let paths: Vec<&str> = following
        .objects
        .implementing(THING)
        .map(|(path, _)| path)
        .collect();
    assert_eq!(paths, [FIRST, SECOND]);
}

#[test]
fn signals_sent_before_the_read_are_not_applied_again() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };
    let service = FakeService::serve(SERVICE, ROOT);
    thing(&service, FIRST, 1);
    service.set_before_next_read(FIRST, THING, "Level", Value::from(2_u32));
    let mut following = Following::start();
    assert_eq!(following.level(FIRST), Some(2));

    assert!(!following.next(), "the change was already in the read");
    assert_eq!(following.level(FIRST), Some(2));

    service.set(FIRST, THING, "Level", Value::from(3_u32));
    assert!(following.next());
    assert_eq!(following.level(FIRST), Some(3));
}

#[test]
fn invalidated_properties_are_read_again() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };
    let service = FakeService::serve(SERVICE, ROOT);
    thing(&service, FIRST, 1);
    let mut following = Following::start();

    service.invalidate(FIRST, THING, "Level", Value::from(7_u32));
    following.until(|objects| {
        objects
            .get(FIRST, THING)
            .and_then(|thing| thing.uint32("Level"))
            == Some(7)
    });
}

#[test]
fn restarted_service_is_read_again() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };
    let first = FakeService::serve(SERVICE, ROOT);
    thing(&first, FIRST, 1);
    let mut following = Following::start();
    assert_eq!(following.level(FIRST), Some(1));

    first.leave();
    following.until(|objects| objects.get(FIRST, THING).is_none());

    let second = FakeService::serve(SERVICE, ROOT);
    thing(&second, SECOND, 2);
    following.until(|objects| objects.get(SECOND, THING).is_some());
    assert_eq!(following.level(SECOND), Some(2));
    assert_eq!(following.level(FIRST), None);
}
