use rikka_text::{
    Glyph, GlyphKey, GlyphPath, GlyphRaster, NOTO_COLOR_EMOJI, NOTO_SANS_JP, TextSpec, Weight,
};
use rikka_text_layout::Synthesis;

const SCALE: f32 = 1.5;
const PEN: (f32, f32) = (10.0, 40.0);
const SAMPLES: usize = 32;

#[derive(Default)]
struct Bounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    current: (f32, f32),
    moves: usize,
    closes: usize,
    drawn: bool,
}

impl Bounds {
    fn new() -> Self {
        Self {
            left: f32::INFINITY,
            top: f32::INFINITY,
            right: f32::NEG_INFINITY,
            bottom: f32::NEG_INFINITY,
            ..Self::default()
        }
    }

    fn include(&mut self, x: f32, y: f32) {
        self.left = self.left.min(x);
        self.top = self.top.min(y);
        self.right = self.right.max(x);
        self.bottom = self.bottom.max(y);
        self.current = (x, y);
        self.drawn = true;
    }

    fn sample(&mut self, point: impl Fn(f32) -> (f32, f32)) {
        for step in 1..=SAMPLES {
            let (x, y) = point(step as f32 / SAMPLES as f32);
            self.include(x, y);
        }
    }

    fn width(&self) -> f32 {
        self.right - self.left
    }
}

impl GlyphPath for Bounds {
    fn move_to(&mut self, x: f32, y: f32) {
        self.moves += 1;
        self.include(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.include(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.current;
        self.sample(|t| {
            let u = 1.0 - t;
            (
                u * u * x0 + 2.0 * u * t * x1 + t * t * x,
                u * u * y0 + 2.0 * u * t * y1 + t * t * y,
            )
        });
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x0, y0) = self.current;
        self.sample(|t| {
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            (
                a * x0 + b * x1 + c * x2 + d * x,
                a * y0 + b * y1 + c * y2 + d * y,
            )
        });
    }

    fn close(&mut self) {
        self.closes += 1;
    }
}

fn glyphs(spec: TextSpec) -> Vec<Glyph> {
    let layout = spec.layout(None);
    layout
        .rows()
        .flat_map(|row| row.glyphs().to_vec())
        .collect()
}

fn origin_for(glyph: &Glyph) -> (f32, f32) {
    (PEN.0 - glyph.x * SCALE, PEN.1 - glyph.y * SCALE)
}

fn outline_bounds(raster: &mut GlyphRaster, glyph: &Glyph) -> Bounds {
    let mut bounds = Bounds::new();
    assert!(raster.outline(glyph, origin_for(glyph), SCALE, &mut bounds));
    assert!(bounds.drawn, "glyph {} has an outline", glyph.id);
    assert_eq!(bounds.moves, bounds.closes, "contours close");
    bounds
}

fn assert_outline_within_raster(raster: &mut GlyphRaster, glyph: &Glyph) -> Bounds {
    let outline = outline_bounds(raster, glyph);
    let (key, x, y) = GlyphKey::place(glyph, origin_for(glyph), SCALE);
    assert_eq!((x, y), (PEN.0 as i32, PEN.1 as i32));
    let image = raster.raster(key).expect("glyph rasters");
    let left = (x + image.left) as f32;
    let top = (y - image.top) as f32;
    let right = left + image.width as f32;
    let bottom = top + image.height as f32;
    for (edge, outline_bounds, rastered) in [
        ("left", outline.left, left),
        ("top", outline.top, top),
        ("right", outline.right, right),
        ("bottom", outline.bottom, bottom),
    ] {
        assert!(
            (outline_bounds - rastered).abs() <= 1.0,
            "glyph {} {edge}: outline {outline_bounds}, raster {rastered}",
            glyph.id
        );
    }
    outline
}

#[test]
fn outline_matches_raster_bounds() {
    let mut raster = GlyphRaster::new();
    for weight in [Weight::NORMAL, Weight::BOLD] {
        for glyph in glyphs(TextSpec::new("Ag永", 24.0).weight(weight))
            .into_iter()
            .chain(glyphs(
                TextSpec::new("Ag永", 24.0)
                    .family(NOTO_SANS_JP)
                    .weight(weight),
            ))
        {
            assert_outline_within_raster(&mut raster, &glyph);
        }
    }
}

#[test]
fn synthesis_applies_to_outline_and_raster() {
    let mut raster = GlyphRaster::new();
    let plain = glyphs(TextSpec::new("H", 48.0))[0];
    let plain_bounds = assert_outline_within_raster(&mut raster, &plain);
    let oblique = Glyph {
        synthesis: Synthesis {
            oblique: true,
            ..Synthesis::NONE
        },
        ..plain
    };
    let oblique_bounds = assert_outline_within_raster(&mut raster, &oblique);
    assert!(oblique_bounds.right > plain_bounds.right + 5.0);
    assert!((oblique_bounds.top - plain_bounds.top).abs() < 0.01);
    let bold = Glyph {
        synthesis: Synthesis {
            bold: true,
            ..Synthesis::NONE
        },
        ..plain
    };
    let bold_bounds = assert_outline_within_raster(&mut raster, &bold);
    assert!(bold_bounds.width() > plain_bounds.width() + 1.0);
    assert!(bold_bounds.top < plain_bounds.top);
}

#[test]
fn color_and_empty_glyphs_have_no_outline() {
    let mut raster = GlyphRaster::new();
    let emoji = glyphs(TextSpec::new("😀", 24.0).family(NOTO_COLOR_EMOJI))[0];
    assert!(emoji.color);
    let mut bounds = Bounds::new();
    assert!(!raster.outline(&emoji, (0.0, 0.0), 1.0, &mut bounds));
    assert!(!bounds.drawn && bounds.moves == 0 && bounds.closes == 0);

    let space = glyphs(TextSpec::new("a b", 24.0))
        .into_iter()
        .find(|glyph| glyph.start == 1)
        .expect("space glyph present");
    let mut bounds = Bounds::new();
    assert!(raster.outline(&space, (0.0, 0.0), 1.0, &mut bounds));
    assert!(!bounds.drawn && bounds.moves == 0 && bounds.closes == 0);
}
