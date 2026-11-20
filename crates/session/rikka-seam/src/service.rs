use crate::call::Call;
use crate::capability::CapabilityId;
use crate::client::SeamClient;
use crate::event::Publisher;
use crate::manifest::Manifest;
use crate::method::Method;

pub trait Service<H = ()>: 'static {
    fn manifest(&self) -> Manifest;

    fn attach(&mut self, seam: ServiceContext) {
        let _ = seam;
    }

    fn call(&mut self, host: &mut H, method: Method, call: Call);
}

#[derive(Clone, Debug)]
pub struct ServiceContext {
    publisher: Publisher,
    client: SeamClient,
}

impl ServiceContext {
    pub(crate) fn new(publisher: Publisher, client: SeamClient) -> Self {
        Self { publisher, client }
    }

    pub fn capability(&self) -> &CapabilityId {
        self.publisher.capability()
    }

    pub fn publisher(&self) -> &Publisher {
        &self.publisher
    }

    pub fn client(&self) -> &SeamClient {
        &self.client
    }
}
