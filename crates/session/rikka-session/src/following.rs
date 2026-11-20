use std::sync::Arc;

use futures_util::stream::{Stream, StreamExt};
use rikka_seam::{CapabilityId, Delivery, Request, SeamClient};
use serde::de::DeserializeOwned;

pub fn changes(
    seam: &SeamClient,
    capability: &'static str,
    name: &'static str,
) -> impl Stream<Item = ()> + Send + 'static {
    let id = CapabilityId::from(capability);
    seam.subscribe_to(capability)
        .filter_map(move |delivery| {
            let id = id.clone();
            async move {
                match delivery {
                    Delivery::Event(event) => (event.name() == name).then_some(()),
                    Delivery::Lagged(lagged) => lagged.includes(&id).then_some(()),
                }
            }
        })
        .boxed()
}

pub fn states<T: DeserializeOwned + Send + 'static>(
    seam: &SeamClient,
    capability: &'static str,
    name: &'static str,
    refresh: impl Fn() -> Request + Send + Sync + 'static,
) -> impl Stream<Item = T> + Send + 'static {
    let id = CapabilityId::from(capability);
    let refresh = Arc::new(refresh);
    let changes = seam.subscribe_to(capability).filter_map({
        let (asking, refresh) = (seam.clone(), Arc::clone(&refresh));
        move |delivery| {
            let (id, asking, refresh) = (id.clone(), asking.clone(), Arc::clone(&refresh));
            async move {
                match delivery {
                    Delivery::Event(event) => (event.name() == name)
                        .then(|| event.decode::<T>().ok())
                        .flatten(),
                    Delivery::Lagged(lagged) => match lagged.includes(&id) {
                        true => fetch(&asking, refresh()).await,
                        false => None,
                    },
                }
            }
        }
    });
    let asking = seam.clone();
    futures_util::stream::once(async move { fetch(&asking, refresh()).await })
        .filter_map(std::future::ready)
        .chain(changes)
        .boxed()
}

async fn fetch<T: DeserializeOwned>(seam: &SeamClient, request: Request) -> Option<T> {
    let capability = request.capability().clone();
    seam.call(request)
        .await
        .and_then(|payload| payload.decode::<T>())
        .inspect_err(|error| tracing::warn!("seam call {capability} failed: {error}"))
        .ok()
}
