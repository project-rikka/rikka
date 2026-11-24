use std::path::Path;
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_image::{Png, Region};

const SIDE: u32 = 512;
const PICTURE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

pub(crate) fn chosen(parent: Option<WindowIdentifier>) -> Result<Option<Arc<[u8]>>, String> {
    let filter = PICTURE_TYPES.iter().fold(
        Filter::named(tr!("discord.space-sheet.pictures").to_string()),
        |filter, mime| filter.mime_type(*mime),
    );
    let selection = Chooser::open(tr!("discord.space-sheet.choose-icon").to_string())
        .parent(parent)
        .filter(filter)
        .run()
        .map_err(|error| error.to_string())?;
    let ChooserResponse::Files(paths) = selection else {
        return Ok(None);
    };
    let Some(path) = paths.first() else {
        return Ok(None);
    };
    squared(path)
        .map(Some)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn squared(path: &Path) -> rikka_image::Result<Arc<[u8]>> {
    let picture = rikka_image::read(path).decode()?;
    let (width, height) = picture.dimensions();
    let side = width.min(height);
    let square = picture.crop(Region::new(
        (width - side) / 2,
        (height - side) / 2,
        side,
        side,
    ))?;
    let fitted = match side > SIDE {
        true => square.resize_exact(SIDE, SIDE, rikka_image::Filter::Lanczos3)?,
        false => square,
    };
    Ok(fitted.write(Png).to_bytes()?.into())
}
