use futures_util::StreamExt;
use rikka_worker::{Broadcast, block_on};

#[test]
fn every_subscriber_hears_what_follows_its_subscription() {
    let broadcast = Broadcast::new();
    let early = broadcast.subscribe();
    broadcast.send(&1);
    let late = broadcast.subscribe();
    drop(broadcast.subscribe());
    broadcast.send(&2);
    drop(broadcast);

    assert_eq!(block_on(early.collect::<Vec<_>>()), [1, 2]);
    assert_eq!(block_on(late.collect::<Vec<_>>()), [2]);
}
