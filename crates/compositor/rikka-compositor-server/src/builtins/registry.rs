use rikka_compositor_protocols::wl_registry::{Request, WlRegistry};

use crate::client::Client;

pub(super) fn request<S>(_state: &mut S, registry: WlRegistry, request: Request<'_>) {
    let Some(client) = Client::from_resource(&registry) else {
        return;
    };
    let Some(core) = client.core() else {
        return;
    };
    match request {
        Request::Bind { name, id } => core.bind(&client, &registry, name, id),
    }
}

pub(super) fn destroyed<S>(_state: &mut S, registry: WlRegistry) {
    if let Some(core) = Client::from_resource(&registry).and_then(|client| client.core()) {
        core.forget_registry(&registry);
    }
}
