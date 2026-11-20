use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_picture::consume::{Content, FeedConsumer, on_arrival};
use rikka_picture::{Feed, Waking};

#[test]
fn a_feed_hands_over_only_its_newest_frame_and_wakes_consumers() {
    let woken = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&woken);
    let wake: Waking = Arc::new(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    on_arrival(&wake);
    let feed = Feed::new();

    feed.show(1, 1, vec![1, 2, 3, 4]);
    feed.show(2, 1, vec![5, 6, 7, 8, 9, 10, 11, 12]);
    feed.show(1, 1, vec![0; 3]);

    assert!(woken.load(Ordering::Relaxed) >= 2);
    let arrival = feed.take().expect("a frame waits");
    assert_eq!((arrival.width, arrival.height), (2, 1));
    assert!(matches!(arrival.content, Content::Pixels(ref pixels) if pixels[0] == 5));
    assert!(feed.take().is_none());
}
