use std::sync::Arc;

use browser_store::FAVICON_MAX_SIDE;
use rikka_image::{ColorScheme, Filter, Image};

use browser_model::Favicon;

const LARGEST: u32 = 1024;

pub(crate) fn decoded(bytes: &[u8], scheme: ColorScheme) -> Option<Favicon> {
    let (image, svg) = rikka_image::read(bytes)
        .max_dimensions(LARGEST, LARGEST)
        .decode()
        .map(|image| (image, None))
        .or_else(|_| drawn(bytes, scheme).map(|image| (image, Some(Arc::from(bytes)))))
        .inspect_err(|error| tracing::debug!(%error, "failed to decode favicon"))
        .ok()?;
    let image = match image.width() > FAVICON_MAX_SIDE || image.height() > FAVICON_MAX_SIDE {
        true => image
            .resize_to_fit(FAVICON_MAX_SIDE, FAVICON_MAX_SIDE, Filter::Lanczos3)
            .inspect_err(|error| tracing::debug!(%error, "failed to resize favicon"))
            .ok()?,
        false => image,
    };
    let (width, height) = (image.width(), image.height());
    (width > 0 && height > 0).then(|| Favicon {
        width,
        height,
        rgba: Arc::from(image.into_rgba8()),
        svg,
    })
}

fn drawn(bytes: &[u8], scheme: ColorScheme) -> rikka_image::Result<Image> {
    rikka_image::read(bytes)
        .color_scheme(scheme)
        .rasterize_svg_to_fit(FAVICON_MAX_SIDE, FAVICON_MAX_SIDE)
}
