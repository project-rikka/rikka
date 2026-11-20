use std::io::Write;
use std::path::Path;

use rikka_image::{Format, Image, Png};
use rikka_types::Size as Pixels;

pub(crate) struct Contents {
    pub(crate) pixels: Pixels,
    pub(crate) text: Vec<(String, String)>,
}

pub(crate) fn read(path: &Path) -> Option<Contents> {
    let info = rikka_image::read(path).format(Format::Png).probe().ok()?;
    Some(Contents {
        pixels: info.size(),
        text: info.text,
    })
}

pub(crate) fn write(
    path: &Path,
    picture: Image,
    fields: &[(String, String)],
) -> std::io::Result<()> {
    let encoded = picture
        .write(Png)
        .text(fields)
        .to_bytes()
        .map_err(std::io::Error::other)?;
    let mut file = rikka_fs::Replace::new(path).private().stage()?;
    file.write_all(&encoded)?;
    file.commit()
}
