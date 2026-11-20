use std::sync::Arc;

use rikka_ui::ImageSource;

const LONGEST: u32 = 480;

pub(crate) fn preview(rgba: &[u8], width: u32, height: u32) -> ImageSource {
    let step = width.max(height).div_ceil(LONGEST).max(1);
    let (across, down) = ((width / step).max(1), (height / step).max(1));
    let mut pixels = Vec::with_capacity((across * down * 4) as usize);
    for row in 0..down {
        for column in 0..across {
            let mut sum = [0u32; 3];
            let mut count = 0;
            for y in row * step..((row + 1) * step).min(height) {
                for x in column * step..((column + 1) * step).min(width) {
                    let at = ((y * width + x) * 4) as usize;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += u32::from(rgba[at + channel]);
                    }
                    count += 1;
                }
            }
            let count = count.max(1);
            pixels.extend(sum.map(|total| ((total + count / 2) / count) as u8));
            pixels.push(u8::MAX);
        }
    }
    ImageSource::Rgba {
        width: across,
        height: down,
        pixels: Arc::from(pixels),
    }
}
