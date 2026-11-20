use std::fmt;
use std::sync::Arc;

use crate::client::packet::Packet;
use crate::client::proxy::Proxy;
use crate::client::queue::QueueShared;

pub(crate) type Tap = Arc<dyn Fn(Packet) + Send + Sync>;

#[derive(Clone)]
pub(crate) enum Destination {
    Queue(Arc<QueueShared>),
    Tap(Tap),
    Discard,
}

#[derive(Clone)]
pub struct Route(pub(crate) Destination);

impl Route {
    pub fn discard() -> Self {
        Self(Destination::Discard)
    }

    pub fn tap<P: Proxy>(tap: impl Fn(P, P::Event) + Send + Sync + 'static) -> Self {
        Self(Destination::Tap(Arc::new(move |packet: Packet| {
            packet.deliver::<P>(&tap);
        })))
    }
}

impl fmt::Debug for Route {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.0 {
            Destination::Queue(_) => "Route(queue)",
            Destination::Tap(_) => "Route(tap)",
            Destination::Discard => "Route(discard)",
        })
    }
}
