use rikka_psd::{
    Color, ColorMode, Depth, Document, lab_decoded, lab_encoded, lab_to_rgb, rgb_to_gray,
    rgb_to_lab,
};
use rikka_types::srgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpaceKind {
    Gray,
    Rgb,
    Cmyk,
    Lab,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Space {
    pub(crate) kind: SpaceKind,
    depth: Depth,
}

impl Space {
    pub(crate) fn from_document(document: &Document) -> Self {
        let kind = match document.color_mode {
            ColorMode::Rgb | ColorMode::Indexed => SpaceKind::Rgb,
            ColorMode::Cmyk => SpaceKind::Cmyk,
            ColorMode::Lab => SpaceKind::Lab,
            ColorMode::Bitmap
            | ColorMode::Grayscale
            | ColorMode::Duotone
            | ColorMode::Multichannel => SpaceKind::Gray,
        };
        Self::new(kind, document.depth)
    }

    pub(crate) fn new(kind: SpaceKind, depth: Depth) -> Self {
        Self { kind, depth }
    }

    pub(crate) fn linear(&self) -> bool {
        self.depth == Depth::ThirtyTwo
    }

    pub(crate) fn channels(&self) -> usize {
        match self.kind {
            SpaceKind::Gray => 1,
            SpaceKind::Rgb | SpaceKind::Lab => 3,
            SpaceKind::Cmyk => 4,
        }
    }

    fn encode_lab(&self, lab: [f64; 3], out: &mut [f32]) {
        let encoded = lab_encoded(lab, self.depth);
        for (slot, value) in out.iter_mut().zip(encoded) {
            *slot = value as f32;
        }
    }

    pub(crate) fn to_rgb(self, pixel: &[f32]) -> [f32; 3] {
        match self.kind {
            SpaceKind::Gray => [pixel[0]; 3],
            SpaceKind::Rgb => [pixel[0], pixel[1], pixel[2]],
            SpaceKind::Cmyk => [
                pixel[0] * pixel[3],
                pixel[1] * pixel[3],
                pixel[2] * pixel[3],
            ],
            SpaceKind::Lab => {
                let [l, a, b] = lab_decoded([pixel[0], pixel[1], pixel[2]], self.depth);
                lab_to_rgb(l, a, b).map(|value| value as f32)
            }
        }
    }

    pub(crate) fn encode_rgb(&self, rgb: [f32; 3], out: &mut [f32]) {
        match self.kind {
            SpaceKind::Gray => out[0] = rgb_to_gray(rgb),
            SpaceKind::Rgb => out[..3].copy_from_slice(&rgb),
            SpaceKind::Cmyk => {
                let k = rgb[0].max(rgb[1]).max(rgb[2]);
                if k <= 0.0 {
                    out[..4].copy_from_slice(&[1.0, 1.0, 1.0, 0.0]);
                } else {
                    out[0] = rgb[0] / k;
                    out[1] = rgb[1] / k;
                    out[2] = rgb[2] / k;
                    out[3] = k;
                }
            }
            SpaceKind::Lab => self.encode_lab(rgb_to_lab(rgb.map(f64::from)), out),
        }
    }

    pub(crate) fn color(&self, color: &Color) -> [f32; 4] {
        let mut out = [0.0f32; 4];
        match (self.kind, color) {
            (SpaceKind::Cmyk, Color::Cmyk { c, m, y, k }) => {
                out = [
                    1.0 - *c as f32,
                    1.0 - *m as f32,
                    1.0 - *y as f32,
                    1.0 - *k as f32,
                ];
            }
            (SpaceKind::Gray, Color::Gray { k }) => out[0] = 1.0 - *k as f32,
            (SpaceKind::Lab, Color::Lab { l, a, b }) => self.encode_lab([*l, *a, *b], &mut out),
            _ => self.authored_rgb(color.to_rgb().map(|value| value as f32), &mut out),
        }
        out
    }

    pub(crate) fn authored_rgb(&self, rgb: [f32; 3], out: &mut [f32]) {
        let rgb = if self.linear() {
            rgb.map(|value| srgb::to_linear_f64(f64::from(value)) as f32)
        } else {
            rgb
        };
        self.encode_rgb(rgb, out);
    }

    pub(crate) fn white(&self) -> [f32; 4] {
        let mut out = [0.0; 4];
        self.encode_rgb([1.0; 3], &mut out);
        out
    }

    pub(crate) fn luminance(&self, pixel: &[f32]) -> f32 {
        match self.kind {
            SpaceKind::Gray => pixel[0],
            SpaceKind::Lab => pixel[0],
            _ => rgb_to_gray(self.to_rgb(pixel)),
        }
    }
}
