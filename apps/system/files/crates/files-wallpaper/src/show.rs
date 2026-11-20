use rikka_seam::net::SocketClient;
use rikka_settings::seam::{CAPABILITY, protocol};
use rikka_settings::session::appearance;

use crate::error::{Error, Result};

pub fn show(id: &str) -> Result<()> {
    let session = SocketClient::for_capability(CAPABILITY).map_err(Error::Session)?;
    session
        .client()
        .call(protocol::set(appearance::DOMAIN, appearance::WALLPAPER, id))
        .blocking_wait()
        .map_err(Error::Refused)?;
    Ok(())
}
