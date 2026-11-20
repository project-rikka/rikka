use rikka_compositor_protocols::wl_callback::WlCallback;
use rikka_compositor_server::Client;

pub(crate) fn dispose(callbacks: impl IntoIterator<Item = WlCallback>) {
    for callback in callbacks {
        if let Some(client) = Client::from_resource(&callback) {
            client.destroy(&callback);
        }
    }
}
