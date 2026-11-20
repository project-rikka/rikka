use rikka_psd::{LineCap, LineJoin, PathData, PathOperation, Rect, Subpath, VectorStroke};
use rikka_types::Point;
use rikka_vector::{
    Area, Cap, Contours, FILL_FLATNESS, FillRule, Join, Path, Rasterizer, Stroke, Stroker,
    Transform,
};

use crate::mask::Mask;

const MAX_FLATTENED: usize = 1 << 22;
const BAND_CELLS: usize = 1 << 20;
const SNAP: f32 = 1e-4;
const NON_ZERO: u16 = 2;

fn outline(path: &mut Path, subpath: &Subpath, width: f32, height: f32) {
    let scaled =
        |point: rikka_psd::Point| Point::new(point.x as f32 * width, point.y as f32 * height);
    let knots = &subpath.knots;
    let Some(first) = knots.first() else {
        return;
    };
    path.move_to(scaled(first.anchor));
    let segments = match subpath.closed {
        true => knots.len(),
        false => knots.len() - 1,
    };
    for index in 0..segments {
        let from = &knots[index];
        let to = &knots[(index + 1) % knots.len()];
        path.cubic_to(
            scaled(from.leaving),
            scaled(to.preceding),
            scaled(to.anchor),
        );
    }
    if subpath.closed {
        path.close();
    }
}

fn coverage(rasterizer: &mut Rasterizer, path: &Path, rule: FillRule, bounds: Rect) -> Mask {
    let mut mask = Mask::new(bounds, 0.0);
    let (width, height) = (mask.width(), mask.height());
    let flattened =
        rasterizer.flatten_within(path, &Transform::IDENTITY, FILL_FLATNESS, MAX_FLATTENED);
    if flattened.is_err() || width == 0 || height == 0 {
        return mask;
    }
    let rows = (BAND_CELLS / width).clamp(1, height);
    for top in (0..height).step_by(rows) {
        let band = rows.min(height - top);
        let area = Area {
            left: bounds.left,
            top: bounds.top + top as i32,
            width: width as u32,
            height: band as u32,
        };
        let out = &mut mask.data[top * width..(top + band) * width];
        if rasterizer
            .coverage(rule, Contours::Overlapping, area, out)
            .is_err()
        {
            out.fill(0.0);
        }
    }
    mask.map(|value| {
        if value < SNAP {
            0.0
        } else if value > 1.0 - SNAP {
            1.0
        } else {
            value
        }
    });
    mask
}

pub(crate) fn rasterize(
    path: &PathData,
    document_width: u32,
    document_height: u32,
    bounds: Rect,
) -> Mask {
    let (width, height) = (document_width as f32, document_height as f32);
    let mut components: Vec<(PathOperation, FillRule, Path)> = Vec::new();
    for subpath in path.subpaths() {
        if subpath.knots.len() < 2 {
            continue;
        }
        let rule = match subpath.rule == NON_ZERO {
            true => FillRule::NonZero,
            false => FillRule::EvenOdd,
        };
        match (subpath.operation, components.last_mut()) {
            (PathOperation::Unspecified(_), Some(last)) => {
                outline(&mut last.2, subpath, width, height);
            }
            (operation, _) => {
                let mut component = Path::new();
                outline(&mut component, subpath, width, height);
                components.push((operation, rule, component));
            }
        }
    }
    let seed = if path.starts_with_all_pixels() && components.is_empty() {
        1.0
    } else {
        0.0
    };
    let mut mask = Mask::new(bounds, seed);
    let mut rasterizer = Rasterizer::new();
    for (index, (operation, rule, component)) in components.iter().enumerate() {
        let plane = coverage(&mut rasterizer, component, *rule, bounds);
        let first = index == 0;
        for (value, coverage) in mask.data.iter_mut().zip(&plane.data) {
            *value = match operation {
                PathOperation::Exclude => *value + coverage - 2.0 * *value * coverage,
                PathOperation::Subtract => {
                    let base = if first { 1.0 - *value } else { *value };
                    (base - coverage).max(0.0)
                }
                PathOperation::Intersect => {
                    let base = if first { 1.0 - *value } else { *value };
                    base * coverage
                }
                PathOperation::Combine | PathOperation::Unspecified(_) => {
                    *value + coverage - *value * coverage
                }
            };
        }
    }
    mask.map(|value| value.clamp(0.0, 1.0));
    mask
}

pub(crate) fn stroke(
    path: &PathData,
    document_width: u32,
    document_height: u32,
    style: &VectorStroke,
    width: f32,
    bounds: Rect,
) -> Mask {
    let mut centre = Path::new();
    for subpath in path.subpaths().filter(|subpath| subpath.knots.len() >= 2) {
        outline(
            &mut centre,
            subpath,
            document_width as f32,
            document_height as f32,
        );
    }
    let dashes: Vec<f32> = style
        .dashes
        .iter()
        .map(|dash| (*dash * style.width) as f32)
        .collect();
    let stroke = Stroke::new(width)
        .join(match style.join {
            LineJoin::Miter => Join::Miter,
            LineJoin::Round => Join::Round,
            LineJoin::Bevel => Join::Bevel,
        })
        .cap(match style.cap {
            LineCap::Butt => Cap::Butt,
            LineCap::Round => Cap::Round,
            LineCap::Square => Cap::Square,
        })
        .miter_limit(style.miter_limit as f32)
        .dashed(&dashes, style.dash_offset as f32);
    let mut stroked = Path::new();
    Stroker::new().outline(&centre, &stroke, FILL_FLATNESS, &mut stroked);
    coverage(&mut Rasterizer::new(), &stroked, FillRule::NonZero, bounds)
}
