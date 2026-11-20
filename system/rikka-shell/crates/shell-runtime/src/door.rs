use rikka_seam::SeamClient;
use rikka_seam::net::{SocketListener, socket_path_beside};

const NAME: &str = "shell";

#[must_use]
pub(crate) fn open(client: SeamClient) -> Option<SocketListener> {
    let Some(path) = socket_path_beside(NAME) else {
        tracing::warn!("no runtime directory, not listening on shell socket");
        return None;
    };
    match SocketListener::serve(&path, client) {
        Ok(listener) => {
            tracing::info!("listening on {}", path.display());
            Some(listener)
        }
        Err(error) => {
            tracing::warn!("failed to listen on shell socket: {error}");
            None
        }
    }
}
