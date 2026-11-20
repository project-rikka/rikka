use rikka_compositor_protocols::wl_display::{Request, WlDisplay};

use crate::client::Client;

pub(super) fn request<S>(_state: &mut S, display: WlDisplay, request: Request) {
    let Some(client) = Client::from_resource(&display) else {
        return;
    };
    let Some(core) = client.core() else {
        return;
    };
    match request {
        Request::Sync { callback } => callback.done(core.serial()),
        Request::GetRegistry { registry } => core.announce(&client, registry),
    }
}
