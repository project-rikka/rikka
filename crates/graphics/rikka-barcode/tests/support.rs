use rikka_barcode::QrCode;

pub(crate) const INK: [u8; 4] = [20, 20, 24, 255];
pub(crate) const PAPER: [u8; 4] = [250, 250, 248, 255];

pub(crate) struct Canvas {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

impl Canvas {
    pub(crate) fn filled(width: u32, height: u32, shade: [u8; 4]) -> Canvas {
        Canvas {
            width,
            height,
            pixels: shade.repeat((width * height) as usize),
        }
    }

    pub(crate) fn code(&mut self, text: &str, at: (u32, u32), module: u32, inverted: bool) -> u32 {
        let code = QrCode::encode(text).expect("text fits in QR code");
        let (dark, light) = match inverted {
            true => (PAPER, INK),
            false => (INK, PAPER),
        };
        let side = code.side() as u32 * module;
        for y in 0..side {
            for x in 0..side {
                let set = code.is_dark((x / module) as usize, (y / module) as usize);
                self.put(at.0 + x, at.1 + y, if set { dark } else { light });
            }
        }
        side
    }

    pub(crate) fn fill(&mut self, at: (u32, u32), size: (u32, u32), shade: [u8; 4]) {
        for y in at.1..at.1 + size.1 {
            for x in at.0..at.0 + size.0 {
                self.put(x, y, shade);
            }
        }
    }

    pub(crate) fn crop(&self, at: (u32, u32), size: (u32, u32)) -> Canvas {
        let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
        for y in at.1..at.1 + size.1 {
            let start = ((y * self.width + at.0) * 4) as usize;
            pixels.extend_from_slice(&self.pixels[start..start + (size.0 * 4) as usize]);
        }
        Canvas {
            width: size.0,
            height: size.1,
            pixels,
        }
    }

    fn put(&mut self, x: u32, y: u32, shade: [u8; 4]) {
        let at = ((y * self.width + x) * 4) as usize;
        self.pixels[at..at + 4].copy_from_slice(&shade);
    }
}
