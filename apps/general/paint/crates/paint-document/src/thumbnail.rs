#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub(crate) struct Canvas {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) struct LayerPixels<'a> {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixel: &'a dyn Fn(usize) -> [f32; 4],
}

impl Canvas {
    pub(crate) fn fitted(&self, longest: u32) -> (u32, u32) {
        let longest = longest.max(1) as f64;
        let scale = (longest / f64::from(self.width.max(self.height).max(1))).min(1.0);
        let width = (f64::from(self.width) * scale).round().max(1.0) as u32;
        let height = (f64::from(self.height) * scale).round().max(1.0) as u32;
        (width, height)
    }

    pub(crate) fn shrink(&self, longest: u32, layer: &LayerPixels<'_>) -> Thumbnail {
        let (width, height) = self.fitted(longest);
        let across = f64::from(self.width) / f64::from(width);
        let down = f64::from(self.height) / f64::from(height);
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            let top = (f64::from(y) * down) as i64;
            let bottom = ((f64::from(y + 1) * down) as i64).max(top + 1);
            for x in 0..width {
                let left = (f64::from(x) * across) as i64;
                let right = ((f64::from(x + 1) * across) as i64).max(left + 1);
                pixels.extend(layer.average(left..right, top..bottom));
            }
        }
        Thumbnail {
            width,
            height,
            pixels,
        }
    }
}

impl LayerPixels<'_> {
    fn average(&self, across: std::ops::Range<i64>, down: std::ops::Range<i64>) -> [u8; 4] {
        let area = ((across.end - across.start) * (down.end - down.start)) as f32;
        let left = i64::from(self.left);
        let top = i64::from(self.top);
        let columns = across.start.max(left)..across.end.min(left + i64::from(self.width));
        let rows = down.start.max(top)..down.end.min(top + i64::from(self.height));
        let mut sum = [0.0f32; 4];
        for row in rows {
            for column in columns.clone() {
                let index = (row - top) as usize * self.width as usize + (column - left) as usize;
                let [r, g, b, a] = (self.pixel)(index);
                sum[0] += r * a;
                sum[1] += g * a;
                sum[2] += b * a;
                sum[3] += a;
            }
        }
        if sum[3] <= 0.0 {
            return [0; 4];
        }
        let alpha = sum[3] / area;
        let channel = |value: f32| (value / sum[3] * 255.0).round().clamp(0.0, 255.0) as u8;
        [
            channel(sum[0]),
            channel(sum[1]),
            channel(sum[2]),
            (alpha * 255.0).round().clamp(0.0, 255.0) as u8,
        ]
    }
}
