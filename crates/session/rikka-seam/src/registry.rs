use std::sync::Arc;

use crate::call::Call;
use crate::capability::{CapabilityId, Version};
use crate::client::SeamClient;
use crate::dispatcher::Dispatcher;
use crate::error::SeamError;
use crate::event::{Delivery, EventBus, Publisher};
use crate::manifest::{Manifest, MethodInfo};
use crate::method::Method;
use crate::service::{Service, ServiceContext};

pub const SEAM_CAPABILITY: &str = "rikka.seam";

pub const CAPABILITIES: &str = "capabilities";

pub struct Seam<H = ()> {
    builtin: Manifest,
    entries: Vec<Entry<H>>,
    bus: EventBus,
    dispatcher: Arc<dyn Dispatcher>,
    beyond: Option<Arc<dyn Dispatcher>>,
}

struct Entry<H> {
    manifest: Arc<Manifest>,
    service: Option<Box<dyn Service<H>>>,
}

#[derive(Clone, Copy)]
enum Target {
    Builtin,
    Entry(usize),
}

struct Routed {
    index: usize,
    method: Method,
    call: Call,
}

impl<H: 'static> Seam<H> {
    pub fn new(dispatcher: impl Dispatcher) -> Self {
        Self {
            builtin: builtin_manifest(),
            entries: Vec::new(),
            bus: EventBus::new(),
            dispatcher: Arc::new(dispatcher),
            beyond: None,
        }
    }

    #[must_use]
    pub fn forwarding_to(mut self, beyond: Arc<dyn Dispatcher>) -> Self {
        self.beyond = Some(beyond);
        self
    }

    pub fn relay(&self, delivery: Delivery) {
        match delivery {
            Delivery::Event(event) => self.bus.send(event),
            Delivery::Lagged(lagged) => self.bus.lagged(&lagged),
        }
    }

    pub fn client(&self) -> SeamClient {
        SeamClient::new(Arc::clone(&self.dispatcher), self.bus.clone())
    }

    pub fn register(&mut self, service: impl Service<H>) -> Result<ServiceContext, SeamError> {
        let mut service: Box<dyn Service<H>> = Box::new(service);
        let manifest = service.manifest();
        manifest.validate()?;
        if self.provides(manifest.capability()) {
            return Err(SeamError::DuplicateCapability {
                capability: manifest.capability().clone(),
            });
        }

        let manifest = Arc::new(manifest);
        let publisher = Publisher::new(Arc::clone(&manifest), self.bus.clone());
        let context = ServiceContext::new(publisher, self.client());
        service.attach(context.clone());

        self.entries.push(Entry {
            manifest,
            service: Some(service),
        });
        Ok(context)
    }

    pub fn manifests(&self) -> impl Iterator<Item = &Manifest> {
        std::iter::once(&self.builtin).chain(self.entries.iter().map(|entry| &*entry.manifest))
    }

    pub fn provides(&self, capability: &CapabilityId) -> bool {
        self.target(capability).is_some()
    }

    pub fn manifest_of(&self, capability: &CapabilityId) -> Option<&Manifest> {
        self.target(capability)
            .map(|target| self.manifest_at(target))
    }

    pub fn dispatch(&mut self, host: &mut H, call: Call) {
        let Some(Routed {
            index,
            method,
            call,
        }) = self.route(call)
        else {
            return;
        };
        match &mut self.entries[index].service {
            Some(service) => service.call(host, method, call),
            None => busy(call),
        }
    }

    pub fn dispatch_within(host: &mut H, call: Call, seam: impl Fn(&mut H) -> &mut Self) {
        let Some(Routed {
            index,
            method,
            call,
        }) = seam(host).route(call)
        else {
            return;
        };
        let Some(mut service) = seam(host).entries[index].service.take() else {
            return busy(call);
        };
        service.call(host, method, call);
        seam(host).entries[index].service = Some(service);
    }

    fn route(&mut self, call: Call) -> Option<Routed> {
        let Some(target) = self.target(call.capability()) else {
            match &self.beyond {
                Some(beyond) => beyond.enqueue(call),
                None => {
                    let capability = call.capability().clone();
                    call.fail(SeamError::UnknownCapability { capability });
                }
            }
            return None;
        };

        let manifest = self.manifest_at(target);
        let provided = manifest.version();
        let declared = manifest.declares_method(call.method().as_str());

        if let Some(required) = call.required_version()
            && !provided.satisfies(required)
        {
            let capability = call.capability().clone();
            call.fail(SeamError::VersionMismatch {
                capability,
                required,
                provided,
            });
            return None;
        }
        if !declared {
            call.unknown_method();
            return None;
        }

        match target {
            Target::Builtin => {
                self.introspect(call);
                None
            }
            Target::Entry(index) => Some(Routed {
                index,
                method: call.method().clone(),
                call,
            }),
        }
    }

    fn introspect(&self, call: Call) {
        let method = call.method().clone();
        match &*method {
            CAPABILITIES => {
                let manifests: Vec<&Manifest> = self.manifests().collect();
                call.encode(&manifests);
            }
            "manifest" => {
                let decoded = call.decode::<ManifestParams>();
                let requested = match decoded {
                    Ok(params) => params.capability,
                    Err(error) => return call.fail(error),
                };
                match self.manifest_of(&requested) {
                    Some(manifest) => call.encode(manifest),
                    None => call.fail(SeamError::UnknownCapability {
                        capability: requested,
                    }),
                }
            }
            _ => call.unknown_method(),
        }
    }

    fn target(&self, capability: &CapabilityId) -> Option<Target> {
        if capability == self.builtin.capability() {
            return Some(Target::Builtin);
        }
        self.entries
            .iter()
            .position(|entry| entry.manifest.capability() == capability)
            .map(Target::Entry)
    }

    fn manifest_at(&self, target: Target) -> &Manifest {
        match target {
            Target::Builtin => &self.builtin,
            Target::Entry(index) => &self.entries[index].manifest,
        }
    }
}

#[derive(serde::Deserialize)]
struct ManifestParams {
    capability: CapabilityId,
}

fn builtin_manifest() -> Manifest {
    Manifest::new(SEAM_CAPABILITY, Version::new(1, 0))
        .summary("The seam's account of itself")
        .method(
            MethodInfo::new(CAPABILITIES)
                .summary("Every capability the desktop currently provides"),
        )
        .method(MethodInfo::new("manifest").summary("The manifest of one named capability"))
}

fn busy(call: Call) {
    let capability = call.capability().clone();
    call.fail(SeamError::Unavailable(format!(
        "`{capability}` is answering another call"
    )));
}

impl<H> std::fmt::Debug for Seam<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seam")
            .field(
                "capabilities",
                &std::iter::once(&self.builtin)
                    .chain(self.entries.iter().map(|entry| &*entry.manifest))
                    .map(Manifest::capability)
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl<H> std::fmt::Debug for Entry<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}
