use std::path::PathBuf;
use std::sync::Arc;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree, fontdb};
use rikka_svg::{Bitmap, Svg};
use rikka_text_catalog::{Family, Query, Weight, catalog};

const FIXTURES: &str = "crates/graphics/rikka-svg/tests/fixtures";
const SCALES: [f32; 2] = [1.0, 2.0];
const OVERSAMPLING: u32 = 4;
const CHANNEL_TOLERANCE: u8 = 24;
const MAX_MEAN_ERROR: f64 = 0.75;
const CHANNELS: usize = 4;
const FIXTURE_FONTS: [(&str, Weight); 4] = [
    ("JetBrains Mono", Weight::NORMAL),
    ("JetBrains Mono", Weight::BOLD),
    ("Noto Sans", Weight::NORMAL),
    ("Noto Color Emoji", Weight::NORMAL),
];

struct Rendering {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

struct Deviation {
    outliers: usize,
    worst: u8,
    mean_error: f64,
}

fn fixtures() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(FIXTURES)
        .expect("fixtures folder")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .collect();
    paths.sort();
    paths
}

fn oracle_fonts() -> Arc<fontdb::Database> {
    let mut database = fontdb::Database::new();
    for (family, weight) in FIXTURE_FONTS {
        let query = Query::new([Family::parse(family)]).weight(weight);
        let face = catalog()
            .query(&query)
            .unwrap_or_else(|| panic!("{family} is not installed"))
            .face();
        let path = catalog()
            .face(face)
            .path()
            .unwrap_or_else(|| panic!("{family} is not a font file"));
        let bytes =
            std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        database.load_font_data(bytes);
    }
    Arc::new(database)
}

fn decoded(data: &[u8]) -> Option<Bitmap> {
    let pixmap = Pixmap::decode_png(data).ok()?;
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let straight = pixel.demultiply();
            [
                straight.red(),
                straight.green(),
                straight.blue(),
                straight.alpha(),
            ]
        })
        .collect();
    Some(Bitmap {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba,
    })
}

fn premultiplied(straight: &[u8]) -> Vec<u8> {
    straight
        .as_chunks::<CHANNELS>()
        .0
        .iter()
        .flat_map(|&[red, green, blue, alpha]| {
            let scaled = |channel: u8| ((u32::from(channel) * u32::from(alpha) + 127) / 255) as u8;
            [scaled(red), scaled(green), scaled(blue), alpha]
        })
        .collect()
}

fn reference(tree: &Tree, width: u32, height: u32, oversampling: u32) -> Rendering {
    let size = tree.size();
    let scale = (width as f32 / size.width()).min(height as f32 / size.height());
    let left = (width as f32 - size.width() * scale) / 2.0;
    let top = (height as f32 - size.height() * scale) / 2.0;
    let mut pixmap =
        Pixmap::new(width * oversampling, height * oversampling).expect("oracle target");
    let finer = oversampling as f32;
    resvg::render(
        tree,
        Transform::from_scale(scale, scale)
            .post_translate(left, top)
            .post_scale(finer, finer),
        &mut pixmap.as_mut(),
    );
    let fine = pixmap.data();
    let (width, height, block) = (width as usize, height as usize, oversampling as usize);
    let stride = width * block * CHANNELS;
    let samples = (block * block) as u32;
    let mut pixels = Vec::with_capacity(width * height * CHANNELS);
    for y in 0..height {
        for x in 0..width {
            let mut sums = [0u32; CHANNELS];
            for row in 0..block {
                let start = (y * block + row) * stride + x * block * CHANNELS;
                let texels = fine[start..start + block * CHANNELS]
                    .as_chunks::<CHANNELS>()
                    .0;
                for texel in texels {
                    for (sum, channel) in sums.iter_mut().zip(texel) {
                        *sum += u32::from(*channel);
                    }
                }
            }
            pixels.extend(sums.map(|sum| ((sum + samples / 2) / samples) as u8));
        }
    }
    Rendering {
        width,
        height,
        pixels,
    }
}

fn outside(value: u8, references: [&Rendering; 2], x: usize, y: usize, channel: usize) -> u8 {
    let (mut low, mut high) = (u8::MAX, u8::MIN);
    for reference in references {
        for near_y in y.saturating_sub(1)..(y + 2).min(reference.height) {
            for near_x in x.saturating_sub(1)..(x + 2).min(reference.width) {
                let near =
                    reference.pixels[(near_y * reference.width + near_x) * CHANNELS + channel];
                low = low.min(near);
                high = high.max(near);
            }
        }
    }
    low.saturating_sub(value).max(value.saturating_sub(high))
}

fn deviation(ours: &[u8], native: &Rendering, supersampled: &Rendering) -> Deviation {
    let (mut outliers, mut worst, mut total) = (0, 0u8, 0u64);
    for y in 0..native.height {
        for x in 0..native.width {
            let at = (y * native.width + x) * CHANNELS;
            let mut farthest = 0u8;
            for channel in 0..CHANNELS {
                let value = ours[at + channel];
                let nearest = value
                    .abs_diff(native.pixels[at + channel])
                    .min(value.abs_diff(supersampled.pixels[at + channel]));
                total += u64::from(nearest);
                farthest = farthest.max(outside(value, [native, supersampled], x, y, channel));
            }
            outliers += usize::from(farthest > CHANNEL_TOLERANCE);
            worst = worst.max(farthest);
        }
    }
    Deviation {
        outliers,
        worst,
        mean_error: total as f64 / (native.width * native.height * CHANNELS) as f64,
    }
}

#[test]
fn renders_like_the_oracle() {
    let decoder = |data: &[u8]| decoded(data);
    let options = Options {
        fontdb: oracle_fonts(),
        ..Options::default()
    };
    let mut failures = Vec::new();
    for path in fixtures() {
        let data = std::fs::read(&path).expect("fixture read");
        let svg = Svg::decoding(&decoder)
            .from_bytes(&data)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let tree = Tree::from_data(&data, &options)
            .unwrap_or_else(|error| panic!("{}: oracle: {error}", path.display()));
        for scale in SCALES {
            let size = svg.size();
            let width = (size.width * scale).ceil() as u32;
            let height = (size.height * scale).ceil() as u32;
            let raster = svg.render(width, height).expect("fixture renders");
            let found = deviation(
                &premultiplied(raster.pixels()),
                &reference(&tree, width, height, 1),
                &reference(&tree, width, height, OVERSAMPLING),
            );
            if found.outliers > 0 || found.mean_error > MAX_MEAN_ERROR {
                failures.push(format!(
                    "{} at {width}x{height}: {} pixels beyond tolerance, worst {}, mean error {:.3}",
                    path.display(),
                    found.outliers,
                    found.worst,
                    found.mean_error,
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
