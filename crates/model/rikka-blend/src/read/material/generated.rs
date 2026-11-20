use super::png;
use crate::file::Record;

const BLANK: i64 = 0;
const GRID: i64 = 1;
const CHECKER: i32 = 32;
const MAX_PIXELS: i64 = 8192 * 8192;

pub(crate) enum Generated {
    Png(Vec<u8>),
    Unsupported,
}

pub(crate) fn generated(image: &Record<'_>) -> Generated {
    let tile = image
        .list("tiles")
        .into_iter()
        .next()
        .filter(|tile| tile.has("gen_x"));
    let source = tile.as_ref().unwrap_or(image);
    let width = source.int("gen_x").unwrap_or(1024);
    let height = source.int("gen_y").unwrap_or(1024);
    if width <= 0 || height <= 0 || width * height > MAX_PIXELS {
        return Generated::Unsupported;
    }
    let (width, height) = (width as u32, height as u32);
    let pixels = match source.int("gen_type").unwrap_or(BLANK) {
        BLANK => {
            let color = source
                .floats::<4>("gen_color")
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let rgba = color.map(|channel| (channel.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            rgba.repeat((width * height) as usize)
        }
        GRID => checker(width, height),
        _ => return Generated::Unsupported,
    };
    match png::encode(width, height, &pixels) {
        Ok(bytes) => Generated::Png(bytes),
        Err(_) => Generated::Unsupported,
    }
}

fn checker(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let half = CHECKER / 2;
    for y in 0..height as i32 {
        let mut dark = (-1i32).pow((y / CHECKER) as u32 % 2);
        let hue_offset = 0.125 * (y / CHECKER) as f32;
        for x in 0..width as i32 {
            if x % CHECKER == 0 {
                dark = -dark;
            }
            let at = ((y as u32 * width + x as u32) * 4) as usize;
            let shade = if dark > 0 { 64 } else { 150 };
            pixels[at..at + 4].copy_from_slice(&[shade, shade, shade, 255]);
            let test_x = ((x % CHECKER) - half).abs();
            let test_y = ((y % CHECKER) - half).abs();
            if test_x < 4 && test_y < 4 && (test_x < 1 || test_y < 1) {
                let hue = (0.125 * (x / CHECKER) as f32 - hue_offset).abs() % 1.0;
                let [red, green, blue] = hsv_to_rgb(hue, 0.9, 0.9);
                pixels[at..at + 4].copy_from_slice(&[
                    (red * 255.0) as u8,
                    (green * 255.0) as u8,
                    (blue * 255.0) as u8,
                    255,
                ]);
            }
        }
    }
    pixels
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let red = ((hue * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let green = (2.0 - (hue * 6.0 - 2.0).abs()).clamp(0.0, 1.0);
    let blue = (2.0 - (hue * 6.0 - 4.0).abs()).clamp(0.0, 1.0);
    [red, green, blue].map(|channel| ((channel - 1.0) * saturation + 1.0) * value)
}
