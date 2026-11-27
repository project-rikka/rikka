use std::sync::Arc;

use browser_store::FAVICON_MAX_SIDE;
use rikka_image::Filter;
use rikka_web::Session;

use browser_model::Favicon;

const LARGEST: u32 = 1024;

pub(crate) async fn fetched(view: Session) -> Option<Favicon> {
    let favicon = view
        .favicon()
        .await
        .inspect_err(|error| tracing::debug!(%error, "failed to read favicon"))
        .ok()?;
    decoded(&favicon.bytes)
}

fn decoded(bytes: &[u8]) -> Option<Favicon> {
    let image = rikka_image::read(bytes)
        .max_dimensions(LARGEST, LARGEST)
        .decode()
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
        svg: None,
    })
}
