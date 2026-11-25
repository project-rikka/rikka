use std::fs;
use std::os::unix::fs::PermissionsExt;

use rikka_compositor_server::{ClientOptions, DisplayHandle, Socket};
use rikka_session::agent;

use crate::state::Compositor;

const DIRECTORY_MODE: u32 = 0o700;

pub(crate) fn listen(display: &DisplayHandle<Compositor>, name: &str) -> Option<Socket> {
    let Some(path) = agent::socket_path_for(name) else {
        tracing::warn!("no runtime directory; the agent cannot drive input");
        return None;
    };
    if let Some(directory) = path.parent() {
        let prepared = fs::create_dir_all(directory).and_then(|()| {
            fs::set_permissions(directory, fs::Permissions::from_mode(DIRECTORY_MODE))
        });
        if let Err(error) = prepared {
            tracing::warn!("cannot prepare {}: {error}", directory.display());
            return None;
        }
    }
    display
        .listen_at(&path, ClientOptions::new().agent())
        .inspect_err(|error| tracing::warn!("cannot listen for the agent's input: {error}"))
        .ok()
}
