pub(crate) mod orientation;

use rikka_image::{Image, Orientation};

use crate::error::{Error, Result};
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avutil::AVRational;
use rikka_video_core::sw::{SwConverter, fitted};

const WIDEST_PIXEL: f64 = 16.0;

pub(crate) struct Shaping {
    pub(crate) stored_aspect: AVRational,
    pub(crate) orientation: Orientation,
    pub(crate) bounds: Option<(u32, u32)>,
}

impl Shaping {
    pub(crate) fn picture(&self, converter: &mut SwConverter, frame: &Frame) -> Result<Image> {
        let (width, height) = (frame.width(), frame.height());
        if width <= 0 || height <= 0 {
            return Err(Error::Unsupported("empty frame".into()));
        }
        let pixel = [frame.sample_aspect_ratio(), self.stored_aspect]
            .into_iter()
            .find_map(ratio)
            .unwrap_or(1.0);
        let shown = displayed((width as u32, height as u32), pixel);
        let bounds = self
            .bounds
            .map(|(across, down)| match self.orientation.swaps_axes() {
                true => (down, across),
                false => (across, down),
            });
        let (out_width, out_height) = fitted(shown, bounds);

        let mut rgba = Vec::new();
        converter.rgba_sized(frame, (out_width, out_height), &mut rgba)?;
        let mut image = Image::from_rgba8(out_width, out_height, rgba)?;
        image.apply_orientation(self.orientation);
        Ok(image)
    }
}

fn ratio(aspect: AVRational) -> Option<f64> {
    (aspect.num > 0 && aspect.den > 0)
        .then(|| f64::from(aspect.num) / f64::from(aspect.den))
        .filter(|pixel| (1.0 / WIDEST_PIXEL..=WIDEST_PIXEL).contains(pixel))
}

fn displayed((width, height): (u32, u32), pixel: f64) -> (u32, u32) {
    let shown = (f64::from(width) * pixel).round().max(1.0);
    (shown.min(f64::from(u32::MAX)) as u32, height)
}
