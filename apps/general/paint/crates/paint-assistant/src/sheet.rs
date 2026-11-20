use paint_document::Edge;
use rikka_image::{Image, Png};
use rikka_text::{GlyphKind, GlyphRaster, TextSpec, Wrap};
use rikka_tool::Reply;
use rikka_tool::{Image as Shown, Media};

use crate::framing::{Framing, GAP};

const CHECKER: u32 = 8;
const LIGHT: [u8; 3] = [255, 255, 255];
const SHADE: [u8; 3] = [214, 214, 214];
const GAP_GREY: [u8; 3] = [96, 96, 96];
const LABEL_SIZE: f32 = 11.0;
const DASH: u32 = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub label: Option<String>,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Showing {
    pub framing: Framing,
    pub panels: Vec<Panel>,
    pub grid: Option<u32>,
    pub outline: Vec<Edge>,
    pub text: String,
}

impl Showing {
    pub fn reply(self) -> Result<Reply, String> {
        let Showing {
            framing,
            panels,
            grid,
            outline,
            text,
        } = self;
        let mut raster = GlyphRaster::new();
        let mut drawn = Vec::with_capacity(panels.len());
        for panel in panels {
            let mut sheet = Sheet::flattened(framing.width, framing.height, &panel.rgba)?;
            if let Some(step) = grid {
                sheet.grid(&framing, step, &mut raster);
            }
            sheet.outline(&framing, &outline);
            if let Some(label) = &panel.label {
                sheet.label(label, 2, i64::from(framing.height) - 16, &mut raster);
            }
            drawn.push(sheet);
        }
        let sheet = Sheet::beside(&drawn);
        let png = sheet.png()?;
        Ok(Reply::new(text).display(Shown::inline(Media::Png, png)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sheet {
    width: u32,
    height: u32,
    rgb: Vec<[u8; 3]>,
}

impl Sheet {
    pub(crate) fn flattened(width: u32, height: u32, rgba: &[u8]) -> Result<Sheet, String> {
        let count = width as usize * height as usize;
        if rgba.len() != count * 4 {
            return Err(format!(
                "a {width}×{height} picture holds {} bytes, not {}",
                count * 4,
                rgba.len()
            ));
        }
        let rgb = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(|(at, pixel)| {
                let (x, y) = (at as u32 % width, at as u32 / width);
                let under = match (x / CHECKER + y / CHECKER) % 2 {
                    0 => LIGHT,
                    _ => SHADE,
                };
                let alpha = u32::from(pixel[3]);
                std::array::from_fn(|channel| {
                    ((u32::from(pixel[channel]) * alpha
                        + u32::from(under[channel]) * (255 - alpha)
                        + 127)
                        / 255) as u8
                })
            })
            .collect();
        Ok(Sheet { width, height, rgb })
    }

    pub(crate) fn beside(sheets: &[Sheet]) -> Sheet {
        let height = sheets.iter().map(|sheet| sheet.height).max().unwrap_or(1);
        let width = sheets.iter().map(|sheet| sheet.width).sum::<u32>()
            + GAP * (sheets.len() as u32).saturating_sub(1);
        let mut joined = Sheet {
            width: width.max(1),
            height,
            rgb: vec![GAP_GREY; width.max(1) as usize * height as usize],
        };
        let mut left = 0;
        for sheet in sheets {
            for y in 0..sheet.height {
                let from = (y * sheet.width) as usize;
                let into = (y * joined.width + left) as usize;
                joined.rgb[into..into + sheet.width as usize]
                    .copy_from_slice(&sheet.rgb[from..from + sheet.width as usize]);
            }
            left += sheet.width + GAP;
        }
        joined
    }

    pub(crate) fn png(&self) -> Result<Vec<u8>, String> {
        let rgba: Vec<u8> = self
            .rgb
            .iter()
            .flat_map(|[r, g, b]| [*r, *g, *b, 255])
            .collect();
        Image::from_rgba8(self.width, self.height, rgba)
            .and_then(|image| image.encode(Png))
            .map_err(|error| format!("the picture could not be encoded: {error}"))
    }

    fn blend(&mut self, x: i64, y: i64, color: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return;
        }
        let pixel = &mut self.rgb[(y as usize) * self.width as usize + x as usize];
        for (channel, laid) in pixel.iter_mut().zip(color) {
            *channel =
                (f32::from(*channel) * (1.0 - alpha) + f32::from(laid) * alpha).round() as u8;
        }
    }

    fn fill(&mut self, left: i64, top: i64, width: i64, height: i64, color: [u8; 3], alpha: f32) {
        for y in top..top + height {
            for x in left..left + width {
                self.blend(x, y, color, alpha);
            }
        }
    }

    fn grid(&mut self, framing: &Framing, step: u32, raster: &mut GlyphRaster) {
        let region = framing.region;
        let first = |start: u32| start.div_ceil(step) * step;
        let mut labels = Vec::new();
        let mut x = first(region.left);
        while x <= region.left + region.width {
            let at = framing.image_x(f64::from(x)).round() as i64;
            self.fill(at, 0, 1, i64::from(self.height), [255, 255, 255], 0.55);
            self.fill(at + 1, 0, 1, i64::from(self.height), [0, 0, 0], 0.35);
            labels.push((x.to_string(), at + 2, 1));
            x += step;
        }
        let mut y = first(region.top);
        while y <= region.top + region.height {
            let at = framing.image_y(f64::from(y)).round() as i64;
            self.fill(0, at, i64::from(self.width), 1, [255, 255, 255], 0.55);
            self.fill(0, at + 1, i64::from(self.width), 1, [0, 0, 0], 0.35);
            if y != region.top {
                labels.push((y.to_string(), 2, at + 2));
            }
            y += step;
        }
        for (text, x, y) in labels {
            self.label(&text, x, y, raster);
        }
    }

    fn outline(&mut self, framing: &Framing, edges: &[Edge]) {
        for edge in edges {
            let from = (
                framing.image_x(f64::from(edge.from.0)).round() as i64,
                framing.image_y(f64::from(edge.from.1)).round() as i64,
            );
            let to = (
                framing.image_x(f64::from(edge.to.0)).round() as i64,
                framing.image_y(f64::from(edge.to.1)).round() as i64,
            );
            let steps = (to.0 - from.0).abs().max((to.1 - from.1).abs());
            for step in 0..=steps {
                let (x, y) = match steps {
                    0 => from,
                    _ => (
                        from.0 + (to.0 - from.0) * step / steps,
                        from.1 + (to.1 - from.1) * step / steps,
                    ),
                };
                let color = match ((x + y).unsigned_abs() / u64::from(DASH)) % 2 {
                    0 => [0, 0, 0],
                    _ => [255, 255, 255],
                };
                self.blend(x, y, color, 1.0);
            }
        }
    }

    fn label(&mut self, text: &str, x: i64, y: i64, raster: &mut GlyphRaster) {
        let laid = TextSpec::new(text.to_owned(), LABEL_SIZE)
            .wrap(Wrap::None)
            .layout(None);
        let size = laid.size();
        self.fill(
            x - 1,
            y,
            size.width.ceil() as i64 + 2,
            size.height.ceil() as i64,
            [0, 0, 0],
            0.6,
        );
        for row in laid.rows() {
            for placed in row.placed((x as f32, y as f32 + row.baseline()), 1.0) {
                let Some(glyph) = raster.raster(placed.key) else {
                    continue;
                };
                let (left, top) = (
                    i64::from(placed.x) + i64::from(glyph.left),
                    i64::from(placed.y) - i64::from(glyph.top),
                );
                let bytes = glyph.kind.bytes_per_pixel();
                for gy in 0..glyph.height as usize {
                    for gx in 0..glyph.width as usize {
                        let at = (gy * glyph.width as usize + gx) * bytes;
                        let coverage = match glyph.kind {
                            GlyphKind::Mask => glyph.pixels[at],
                            GlyphKind::Color => glyph.pixels[at + 3],
                        };
                        if coverage > 0 {
                            self.blend(
                                left + gx as i64,
                                top + gy as i64,
                                [255, 255, 255],
                                f32::from(coverage) / 255.0,
                            );
                        }
                    }
                }
            }
        }
    }
}
