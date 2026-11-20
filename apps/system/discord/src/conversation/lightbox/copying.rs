use std::sync::Arc;

use discord_core::Picture;
use rikka_app::prelude::*;
use rikka_image::{Format, Png};
use rikka_ui::Offer;
use rikka_ui::clipboard::publish;

use super::face::LOADED_AT;
use crate::conversation::Media;
use crate::pictures;

pub(super) fn copy_picture(picture: Picture) {
    spawn_blocking(
        move || {
            pictures::load(&picture, LOADED_AT)
                .map_err(|error| error.to_string())
                .and_then(|bytes| png_of(bytes).map_err(|error| error.to_string()))
        },
        |media: &mut Media, png: Result<Arc<[u8]>, String>| {
            let published =
                png.and_then(|png| publish(Offer::png(png)).map_err(|error| error.to_string()));
            match published {
                Ok(()) => skip_view(),
                Err(reason) => media.show_notice(
                    tr!("discord.conversation.copy-failed", reason = reason.as_str()).to_string(),
                ),
            }
        },
    );
}

fn png_of(bytes: Arc<[u8]>) -> rikka_image::Result<Arc<[u8]>> {
    if rikka_image::read(&bytes[..]).probe()?.format == Format::Png {
        return Ok(bytes);
    }
    let png = rikka_image::read(&bytes[..]).decode()?.encode(Png)?;
    Ok(Arc::from(png))
}
