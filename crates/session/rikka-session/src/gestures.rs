use futures_util::stream::{Stream, StreamExt};
use rikka_seam::{Delivery, Event, SeamClient, Version};
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.gestures", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const SWIPED: &str = "swiped";

pub const FINGERS: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Swipe {
    Began { axis: Axis, output: Option<String> },
    Moved { dx: f64, dy: f64, time: u32 },
    Ended { cancelled: bool, time: u32 },
}

#[must_use]
pub fn swiped(event: &Event) -> Option<Swipe> {
    PROTOCOL.decode_event(event, SWIPED)
}

pub fn swipes(seam: &SeamClient) -> impl Stream<Item = Swipe> + Send + 'static {
    seam.subscribe_to(CAPABILITY)
        .filter_map(|delivery| async move {
            match delivery {
                Delivery::Event(event) => swiped(&event),
                Delivery::Lagged(_) => None,
            }
        })
        .boxed()
}
