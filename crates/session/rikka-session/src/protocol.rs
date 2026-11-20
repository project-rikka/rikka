use futures_util::Stream;
use rikka_seam::{Event, Reply, Request, SeamClient, Version};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Protocol {
    capability: &'static str,
    version: Version,
}

impl Protocol {
    #[must_use]
    pub const fn new(capability: &'static str, version: Version) -> Protocol {
        Protocol {
            capability,
            version,
        }
    }

    #[must_use]
    pub const fn capability(self) -> &'static str {
        self.capability
    }

    #[must_use]
    pub const fn version(self) -> Version {
        self.version
    }

    #[must_use]
    pub fn method(self, method: &'static str) -> Request {
        Request::new(self.capability, method).require(self.version)
    }

    #[must_use]
    pub fn request_with<T: Serialize + ?Sized>(self, method: &'static str, body: &T) -> Request {
        self.method(method).encode(body)
    }

    pub fn call(self, seam: &SeamClient, method: &'static str) -> Reply {
        seam.call(self.method(method))
    }

    pub fn call_with<T: Serialize + ?Sized>(
        self,
        seam: &SeamClient,
        method: &'static str,
        body: &T,
    ) -> Reply {
        seam.call(self.request_with(method, body))
    }

    pub fn notify(self, seam: &SeamClient, method: &'static str) {
        seam.notify(self.method(method));
    }

    pub fn notify_with<T: Serialize + ?Sized>(
        self,
        seam: &SeamClient,
        method: &'static str,
        body: &T,
    ) {
        seam.notify(self.request_with(method, body));
    }

    #[must_use]
    pub fn decode_event<T: DeserializeOwned>(self, event: &Event, name: &str) -> Option<T> {
        (event.capability().as_str() == self.capability && event.name() == name)
            .then(|| event.decode().ok())
            .flatten()
    }

    pub fn changes(
        self,
        seam: &SeamClient,
        announced: &'static str,
    ) -> impl Stream<Item = ()> + Send + 'static {
        crate::following::changes(seam, self.capability, announced)
    }

    pub fn states<T: DeserializeOwned + Send + 'static>(
        self,
        seam: &SeamClient,
        announced: &'static str,
        refresh: &'static str,
    ) -> impl Stream<Item = T> + Send + 'static {
        crate::following::states(seam, self.capability, announced, move || {
            self.method(refresh)
        })
    }
}
