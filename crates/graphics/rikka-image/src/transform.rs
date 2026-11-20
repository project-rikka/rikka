use crate::orientation::Orientation;
use crate::pixels::{Pixels, Sample, by_depth};
use crate::raster::Image;

impl Image {
    pub fn apply_orientation(&mut self, orientation: Orientation) {
        if orientation == Orientation::Upright {
            return;
        }
        let (width, height) = self.dimensions();
        let channels = usize::from(self.color().channel_count());
        let size = (width as usize, height as usize);
        let pixels = by_depth!(&self.pixels, samples => {
            oriented(samples, size, channels, orientation)
        });
        match orientation.swaps_axes() {
            true => self.replace_pixels(height, width, pixels),
            false => self.replace_pixels(width, height, pixels),
        }
    }
}

fn oriented<S: Sample>(
    samples: &[S],
    (width, height): (usize, usize),
    channels: usize,
    orientation: Orientation,
) -> Pixels {
    let (out_width, out_height) = match orientation.swaps_axes() {
        true => (height, width),
        false => (width, height),
    };
    let source_of = |x: usize, y: usize| -> (usize, usize) {
        match orientation {
            Orientation::Upright => (x, y),
            Orientation::Rotate90 => (y, height - 1 - x),
            Orientation::Rotate180 => (width - 1 - x, height - 1 - y),
            Orientation::Rotate270 => (width - 1 - y, x),
            Orientation::FlipHorizontal => (width - 1 - x, y),
            Orientation::FlipVertical => (x, height - 1 - y),
            Orientation::Rotate90FlipHorizontal => (y, x),
            Orientation::Rotate270FlipHorizontal => (width - 1 - y, height - 1 - x),
        }
    };
    let mut out = Vec::with_capacity(samples.len());
    for y in 0..out_height {
        for x in 0..out_width {
            let (source_x, source_y) = source_of(x, y);
            let at = (source_y * width + source_x) * channels;
            out.extend_from_slice(&samples[at..at + channels]);
        }
    }
    S::wrap(out)
}
