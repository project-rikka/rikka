use crate::support::{ink, pixel};
use rikka_simd::Level;
use rikka_test_fonts::font_bytes;
use rikka_text_font::OutlineSink;
use rikka_text_font::{Font, Transform};
use rikka_text_raster::{GlyphImage, GlyphImageKind, GlyphRequest, Outline, Rasterizer};

fn rectangle(outline: &mut Outline, x0: f32, y0: f32, x1: f32, y1: f32) {
    outline.move_to(x0, y0);
    outline.line_to(x1, y0);
    outline.line_to(x1, y1);
    outline.line_to(x0, y1);
    outline.close();
}

fn fill(outline: &Outline) -> GlyphImage {
    let mut image = GlyphImage::new();
    assert_eq!(
        Rasterizer::new().fill(outline, &Transform::IDENTITY, &mut image),
        Ok(())
    );
    image
}

fn quadratic_circle(outline: &mut Outline, cx: f32, cy: f32, radius: f32, segments: usize) -> f64 {
    let step = std::f32::consts::TAU / segments as f32;
    let reach = radius / (step * 0.5).cos();
    let point = |angle: f32, length: f32| (cx + length * angle.cos(), cy + length * angle.sin());
    let start = point(0.0, radius);
    outline.move_to(start.0, start.1);
    let mut area = 0.0f64;
    let mut previous = start;
    for index in 0..segments {
        let control = point((index as f32 + 0.5) * step, reach);
        let end = point((index as f32 + 1.0) * step, radius);
        outline.quad_to(control.0, control.1, end.0, end.1);
        let cross = |a: (f32, f32), b: (f32, f32)| {
            f64::from(a.0) * f64::from(b.1) - f64::from(b.0) * f64::from(a.1)
        };
        area += 0.5 * cross(previous, end);
        let triangle = 0.5
            * (f64::from(control.0 - previous.0) * f64::from(end.1 - previous.1)
                - f64::from(end.0 - previous.0) * f64::from(control.1 - previous.1));
        area += 2.0 / 3.0 * triangle;
        previous = end;
    }
    area.abs()
}

#[test]
fn rectangle_coverage_sums_to_area() {
    let mut outline = Outline::new();
    rectangle(&mut outline, 0.3, 0.7, 10.6, 5.2);
    let image = fill(&outline);
    assert_eq!(image.kind(), GlyphImageKind::Mask);
    assert_eq!((image.left(), image.top()), (0, 6));
    assert_eq!((image.width(), image.height()), (11, 6));
    let area = 10.3 * 4.5;
    assert!(
        (ink(&image) - area).abs() < 0.1,
        "{} vs {area}",
        ink(&image)
    );
    for (x, y, expected) in [
        (5, 3, 255),
        (0, 3, 178),
        (10, 3, 153),
        (5, 0, 51),
        (5, 5, 77),
    ] {
        let value = i32::from(pixel(&image, x, y)[0]);
        assert!((value - expected).abs() <= 1, "pixel {x},{y} is {value}");
    }
}

#[test]
fn quadratic_circle_covers_analytic_area() {
    for (radius, segments) in [(3.3f32, 8), (20.25, 8), (41.0, 16)] {
        let mut outline = Outline::new();
        let area = quadratic_circle(&mut outline, 50.4, 60.7, radius, segments);
        let image = fill(&outline);
        let flattening = f64::from(std::f32::consts::TAU * radius) / 16.0;
        assert!(
            (ink(&image) - area).abs() < flattening + 0.1,
            "radius {radius}: {} vs {area}",
            ink(&image)
        );
    }
}

#[test]
fn opposite_winding_contour_cuts_hole() {
    let mut outline = Outline::new();
    rectangle(&mut outline, 0.0, 0.0, 10.0, 10.0);
    outline.move_to(2.0, 2.0);
    outline.line_to(2.0, 8.0);
    outline.line_to(8.0, 8.0);
    outline.line_to(8.0, 2.0);
    let image = fill(&outline);
    assert_eq!(pixel(&image, 5, 5), [0]);
    assert_eq!(pixel(&image, 1, 1), [255]);
    assert!((ink(&image) - 64.0).abs() < 1e-3);
}

#[test]
fn every_simd_level_rasterizes_identical_masks() {
    let mut scalar = Rasterizer::with_level(Level::baseline());
    let mut wide: Vec<Rasterizer> = Level::supported()
        .skip(1)
        .map(Rasterizer::with_level)
        .collect();
    let mut expected = GlyphImage::new();
    let mut actual = GlyphImage::new();
    let mut drawn_images = 0;
    for file in [
        "InterVariable.ttf",
        "NotoSansCJK-VF.otf.ttc",
        "Font Awesome 7 Free-Solid-900.otf",
    ] {
        let data = font_bytes(file);
        let font = Font::new(&data).unwrap();
        let glyphs = font.glyph_count().min(400);
        for id in (0..glyphs).step_by(3) {
            for ppem in [9.5, 12.0, 16.0, 23.7, 48.0, 61.0] {
                let request =
                    GlyphRequest::new(rikka_text_font::GlyphId(id), ppem).subpixel_x(0.25);
                let drawn = scalar.render_outline(&font, &request, &mut expected);
                drawn_images += usize::from(drawn.is_ok() && !expected.is_empty());
                for rasterizer in &mut wide {
                    assert_eq!(
                        rasterizer.render_outline(&font, &request, &mut actual),
                        drawn
                    );
                    assert_eq!(
                        actual,
                        expected,
                        "{file} glyph {id} at {ppem} with {:?}",
                        rasterizer.level()
                    );
                }
            }
        }
    }
    assert!(
        drawn_images > 1500,
        "{drawn_images} glyph images were drawn"
    );
    let mut outline = Outline::new();
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 10_000) as f32 / 100.0
    };
    for _ in 0..50 {
        outline.clear();
        outline.move_to(next(), next());
        for _ in 0..12 {
            outline.quad_to(next(), next(), next(), next());
            outline.line_to(next(), next());
        }
        let filled = scalar.fill(&outline, &Transform::IDENTITY, &mut expected);
        for rasterizer in &mut wide {
            assert_eq!(
                rasterizer.fill(&outline, &Transform::IDENTITY, &mut actual),
                filled
            );
            assert_eq!(actual, expected);
        }
    }
}
