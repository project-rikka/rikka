use rikka_test_fonts::Writer;

#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub(crate) extend: u8,
    pub(crate) stops: Vec<(f32, u16, f32)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Paint {
    Solid(u16, f32),
    Linear([i16; 6], Line),
    Radial([i16; 6], Line),
    Sweep([i16; 2], f32, f32, Line),
    Glyph(u16, Box<Paint>),
    ColrGlyph(u16),
    Translate(i16, i16, Box<Paint>),
    Scale(f32, f32, Box<Paint>),
    Composite(Box<Paint>, u8, Box<Paint>),
    Layers(Vec<Paint>),
}

pub(crate) struct ColorGlyph {
    pub(crate) glyph: u16,
    pub(crate) paint: Paint,
    pub(crate) clip: Option<[i16; 4]>,
}

struct Builder {
    w: Writer,
    layers: Vec<usize>,
}

impl Builder {
    fn paint(&mut self, paint: &Paint) -> usize {
        let start = self.w.len();
        match paint {
            Paint::Solid(index, alpha) => {
                self.w.u8(2).u16(*index).f2dot14(*alpha);
            }
            Paint::Linear(points, line) | Paint::Radial(points, line) => {
                let format = if matches!(paint, Paint::Linear(..)) {
                    4
                } else {
                    6
                };
                self.w.u8(format);
                let line_at = self.w.len();
                self.w.u24(0);
                for &value in points {
                    self.w.i16(value);
                }
                let line_start = self.line(line);
                self.w.set_u24(line_at, line_start - start);
            }
            Paint::Sweep(center, start_degrees, end_degrees, line) => {
                self.w.u8(8);
                let line_at = self.w.len();
                self.w
                    .u24(0)
                    .i16(center[0])
                    .i16(center[1])
                    .f2dot14(start_degrees / 180.0 - 1.0)
                    .f2dot14(end_degrees / 180.0 - 1.0);
                let line_start = self.line(line);
                self.w.set_u24(line_at, line_start - start);
            }
            Paint::Glyph(glyph, child) => {
                self.w.u8(10);
                let child_at = self.w.len();
                self.w.u24(0).u16(*glyph);
                let child_start = self.paint(child);
                self.w.set_u24(child_at, child_start - start);
            }
            Paint::ColrGlyph(glyph) => {
                self.w.u8(11).u16(*glyph);
            }
            Paint::Translate(dx, dy, child) => {
                self.w.u8(14);
                let child_at = self.w.len();
                self.w.u24(0).i16(*dx).i16(*dy);
                let child_start = self.paint(child);
                self.w.set_u24(child_at, child_start - start);
            }
            Paint::Scale(sx, sy, child) => {
                self.w.u8(16);
                let child_at = self.w.len();
                self.w.u24(0).f2dot14(*sx).f2dot14(*sy);
                let child_start = self.paint(child);
                self.w.set_u24(child_at, child_start - start);
            }
            Paint::Composite(source, mode, backdrop) => {
                self.w.u8(32);
                let source_at = self.w.len();
                self.w.u24(0).u8(*mode);
                let backdrop_at = self.w.len();
                self.w.u24(0);
                let source_start = self.paint(source);
                self.w.set_u24(source_at, source_start - start);
                let backdrop_start = self.paint(backdrop);
                self.w.set_u24(backdrop_at, backdrop_start - start);
            }
            Paint::Layers(children) => {
                let first = self.layers.len();
                self.layers.resize(first + children.len(), 0);
                self.w.u8(1).u8(children.len() as u8).u32(first as u32);
                for (index, child) in children.iter().enumerate() {
                    self.layers[first + index] = self.paint(child);
                }
            }
        }
        start
    }

    fn line(&mut self, line: &Line) -> usize {
        let start = self.w.len();
        self.w.u8(line.extend).u16(line.stops.len() as u16);
        for &(offset, index, alpha) in &line.stops {
            self.w.f2dot14(offset).u16(index).f2dot14(alpha);
        }
        start
    }
}

pub(crate) fn colr(glyphs: &[ColorGlyph], layers_v0: &[(u16, Vec<(u16, u16)>)]) -> Vec<u8> {
    let mut b = Builder {
        w: Writer::new(),
        layers: Vec::new(),
    };
    b.w.u16(1)
        .u16(layers_v0.len() as u16)
        .u32(0)
        .u32(0)
        .u16(
            layers_v0
                .iter()
                .map(|(_, layers)| layers.len() as u16)
                .sum(),
        )
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0);
    let list = b.w.len();
    b.w.set_u32(14, list);
    b.w.u32(glyphs.len() as u32);
    let mut records = Vec::new();
    for glyph in glyphs {
        records.push(b.w.len());
        b.w.u16(glyph.glyph).u32(0);
    }
    let layer_count: usize = glyphs.iter().map(|glyph| layer_count(&glyph.paint)).sum();
    let layer_list = b.w.len();
    if layer_count > 0 {
        b.w.set_u32(18, layer_list);
        b.w.u32(layer_count as u32);
        for _ in 0..layer_count {
            b.w.u32(0);
        }
    }
    for (glyph, record) in glyphs.iter().zip(records) {
        let start = b.paint(&glyph.paint);
        b.w.set_u32(record + 2, start - list);
    }
    for (index, paint) in b.layers.clone().into_iter().enumerate() {
        b.w.set_u32(layer_list + 4 + index * 4, paint - layer_list);
    }
    let clipped: Vec<&ColorGlyph> = glyphs.iter().filter(|glyph| glyph.clip.is_some()).collect();
    if !clipped.is_empty() {
        let clip_list = b.w.len();
        b.w.set_u32(22, clip_list);
        b.w.u8(1).u32(clipped.len() as u32);
        let mut boxes = Vec::new();
        for glyph in &clipped {
            b.w.u16(glyph.glyph).u16(glyph.glyph);
            boxes.push(b.w.len());
            b.w.u24(0);
        }
        for (glyph, at) in clipped.iter().zip(boxes) {
            let start = b.w.len();
            b.w.set_u24(at, start - clip_list);
            b.w.u8(1);
            for value in glyph.clip.unwrap() {
                b.w.i16(value);
            }
        }
    }
    let base_records = b.w.len();
    b.w.set_u32(4, base_records);
    let mut first = 0u16;
    for (glyph, layers) in layers_v0 {
        b.w.u16(*glyph).u16(first).u16(layers.len() as u16);
        first += layers.len() as u16;
    }
    let layer_records = b.w.len();
    b.w.set_u32(8, layer_records);
    for (_, layers) in layers_v0 {
        for &(glyph, index) in layers {
            b.w.u16(glyph).u16(index);
        }
    }
    b.w.finish()
}

pub(crate) fn cpal(colors: &[[u8; 4]]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u16(0)
        .u16(colors.len() as u16)
        .u16(1)
        .u16(colors.len() as u16)
        .u32(14)
        .u16(0);
    for &[r, g, b, a] in colors {
        w.u8(b).u8(g).u8(r).u8(a);
    }
    w.finish()
}

fn layer_count(paint: &Paint) -> usize {
    match paint {
        Paint::Layers(children) => children.len() + children.iter().map(layer_count).sum::<usize>(),
        Paint::Glyph(_, child) | Paint::Translate(_, _, child) | Paint::Scale(_, _, child) => {
            layer_count(child)
        }
        Paint::Composite(source, _, backdrop) => layer_count(source) + layer_count(backdrop),
        _ => 0,
    }
}
