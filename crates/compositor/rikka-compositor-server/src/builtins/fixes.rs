use rikka_compositor_protocols::wl_fixes::{Request, WlFixes};

use crate::client::Client;

pub(super) fn request<S>(_state: &mut S, fixes: WlFixes, request: Request) {
    let Some(client) = Client::from_resource(&fixes) else {
        return;
    };
    match request {
        Request::Destroy => {}
        Request::DestroyRegistry { registry } => client.destroy(&registry),
        Request::AckGlobalRemove { registry, name } => {
            if let Some(core) = client.core() {
                core.acknowledge(&fixes, &registry, name);
            }
        }
    }
}
