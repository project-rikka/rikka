use rikka_vector::{Canvas, Paint, Pattern, Pixmap, Sampling, Tiling, Transform};

use crate::parse::values::{ViewBox, fit};
use crate::render::{Host, draw_clipped, draw_group, fill_rect};
use crate::tree::{Picture, PictureContent};

const SHRINK_BELOW: f32 = 0.5;

pub(crate) fn draw(picture: &Picture, parent: &Transform, host: Host, canvas: &mut Canvas) {
    let transform = picture.transform.then(*parent);
    let natural = match &picture.content {
        PictureContent::Raster(pixmap) => (pixmap.width() as f32, pixmap.height() as f32),
        PictureContent::Vector(tree) => (tree.width, tree.height),
    };
    let view = ViewBox {
        x: 0.0,
        y: 0.0,
        width: natural.0,
        height: natural.1,
    };
    let frame = picture.frame;
    let placed = fit(view, picture.aspect, frame.width(), frame.height())
        .then(Transform::translate(frame.min_x(), frame.min_y()));
    let (window, rule) = fill_rect(frame);
    match &picture.content {
        PictureContent::Raster(pixmap) => {
            let scale = placed.then(transform).max_scale();
            let (reduced, factor) = reduced(pixmap, scale);
            let pixmap = reduced.as_ref().unwrap_or(pixmap);
            let paint = Paint::Pattern(Pattern {
                pixmap,
                transform: Transform::scale(factor, factor).then(placed),
                tiling: Tiling::Once,
                sampling: match picture.smooth {
                    true => Sampling::Bilinear,
                    false => Sampling::Nearest,
                },
                opacity: 1.0,
            });
            if let Err(error) = canvas.fill(&window, &transform, rule, &paint) {
                tracing::debug!("skipping SVG image: {error}");
            }
        }
        PictureContent::Vector(tree) => {
            draw_clipped(canvas, &window, &transform, rule, |canvas| {
                draw_group(&tree.root, &placed.then(transform), host, canvas);
            });
        }
    }
}

fn reduced(pixmap: &Pixmap, scale: f32) -> (Option<Pixmap>, f32) {
    let mut factor = 1u32;
    while scale * (factor as f32) < SHRINK_BELOW
        && pixmap.width() / (factor * 2) >= 1
        && pixmap.height() / (factor * 2) >= 1
    {
        factor *= 2;
    }
    if factor == 1 {
        return (None, 1.0);
    }
    let (width, height) = (pixmap.width() / factor, pixmap.height() / factor);
    let Ok(mut small) = Pixmap::new(width, height) else {
        return (None, 1.0);
    };
    let source = pixmap.data();
    let stride = pixmap.width() as usize * 4;
    let count = factor * factor;
    for (index, pixel) in small
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let (x, y) = (index as u32 % width, index as u32 / width);
        let mut sums = [0u32; 4];
        for row in 0..factor {
            let start = (y * factor + row) as usize * stride + (x * factor) as usize * 4;
            for texel in source[start..start + factor as usize * 4]
                .as_chunks::<4>()
                .0
            {
                for (sum, channel) in sums.iter_mut().zip(texel) {
                    *sum += u32::from(*channel);
                }
            }
        }
        *pixel = sums.map(|sum| ((sum + count / 2) / count) as u8);
    }
    (Some(small), factor as f32)
}
