use std::sync::Arc;

use launcher_auth::SkinVariant;

const SKIN: u32 = 64;
const BODY_WIDTH: u32 = 16;
const BODY_HEIGHT: u32 = 32;
const CAPE_WIDTH: u32 = 10;
const CAPE_HEIGHT: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rendered {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Arc<[u8]>,
}

struct Texture {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Patch {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

const fn patch(x: u32, y: u32, width: u32, height: u32) -> Patch {
    Patch {
        x,
        y,
        width,
        height,
    }
}

impl Texture {
    fn decode(bytes: &[u8]) -> Result<Texture, String> {
        let image = rikka_image::read(bytes)
            .decode()
            .map_err(|error| error.to_string())?;
        let (width, height) = image.dimensions();
        Ok(Texture {
            width,
            height,
            pixels: image.into_rgba8(),
        })
    }

    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0; 4];
        }
        let at = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[at],
            self.pixels[at + 1],
            self.pixels[at + 2],
            self.pixels[at + 3],
        ]
    }

    fn scale(&self) -> u32 {
        (self.width / SKIN).max(1)
    }
}

impl Canvas {
    fn new(width: u32, height: u32) -> Canvas {
        Canvas {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    fn draw(
        &mut self,
        texture: &Texture,
        from: Patch,
        x: u32,
        y: u32,
        mirrored: bool,
        overlay: bool,
    ) {
        let scale = texture.scale();
        for row in 0..from.height {
            for column in 0..from.width {
                let source_column = match mirrored {
                    true => from.width - 1 - column,
                    false => column,
                };
                let [red, green, blue, alpha] =
                    texture.pixel((from.x + source_column) * scale, (from.y + row) * scale);
                let (target_x, target_y) = (x + column, y + row);
                if target_x >= self.width || target_y >= self.height {
                    continue;
                }
                let at = ((target_y * self.width + target_x) * 4) as usize;
                match (overlay, alpha) {
                    (true, 0) => {}
                    (true, alpha) => {
                        let cover = u32::from(alpha);
                        for (channel, value) in [red, green, blue].into_iter().enumerate() {
                            let under = u32::from(self.pixels[at + channel]);
                            self.pixels[at + channel] =
                                ((u32::from(value) * cover + under * (255 - cover)) / 255) as u8;
                        }
                        self.pixels[at + 3] = self.pixels[at + 3].max(alpha);
                    }
                    (false, _) => {
                        self.pixels[at..at + 4].copy_from_slice(&[red, green, blue, 255]);
                    }
                }
            }
        }
    }

    fn enlarged(self, factor: u32) -> Rendered {
        let width = self.width * factor;
        let height = self.height * factor;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let from = (((y / factor) * self.width + x / factor) * 4) as usize;
                let to = ((y * width + x) * 4) as usize;
                pixels[to..to + 4].copy_from_slice(&self.pixels[from..from + 4]);
            }
        }
        Rendered {
            width,
            height,
            pixels: pixels.into(),
        }
    }
}

pub(crate) fn face(bytes: &[u8], factor: u32) -> Result<Rendered, String> {
    let texture = Texture::decode(bytes)?;
    let mut canvas = Canvas::new(8, 8);
    canvas.draw(&texture, patch(8, 8, 8, 8), 0, 0, false, false);
    canvas.draw(&texture, patch(40, 8, 8, 8), 0, 0, false, true);
    Ok(canvas.enlarged(factor))
}

pub(crate) fn body(bytes: &[u8], variant: SkinVariant, factor: u32) -> Result<Rendered, String> {
    let texture = Texture::decode(bytes)?;
    let modern = texture.height >= texture.width;
    let arm = match variant {
        SkinVariant::Classic => 4,
        SkinVariant::Slim => 3,
    };
    let mut canvas = Canvas::new(BODY_WIDTH, BODY_HEIGHT);
    canvas.draw(&texture, patch(8, 8, 8, 8), 4, 0, false, false);
    canvas.draw(&texture, patch(20, 20, 8, 12), 4, 8, false, false);
    canvas.draw(&texture, patch(44, 20, arm, 12), 4 - arm, 8, false, false);
    canvas.draw(&texture, patch(4, 20, 4, 12), 4, 20, false, false);
    match modern {
        true => {
            canvas.draw(&texture, patch(36, 52, arm, 12), 12, 8, false, false);
            canvas.draw(&texture, patch(20, 52, 4, 12), 8, 20, false, false);
            canvas.draw(&texture, patch(40, 8, 8, 8), 4, 0, false, true);
            canvas.draw(&texture, patch(20, 36, 8, 12), 4, 8, false, true);
            canvas.draw(&texture, patch(44, 36, arm, 12), 4 - arm, 8, false, true);
            canvas.draw(&texture, patch(52, 52, arm, 12), 12, 8, false, true);
            canvas.draw(&texture, patch(4, 36, 4, 12), 4, 20, false, true);
            canvas.draw(&texture, patch(4, 52, 4, 12), 8, 20, false, true);
        }
        false => {
            canvas.draw(&texture, patch(44, 20, arm, 12), 12, 8, true, false);
            canvas.draw(&texture, patch(4, 20, 4, 12), 8, 20, true, false);
            canvas.draw(&texture, patch(40, 8, 8, 8), 4, 0, false, true);
        }
    }
    Ok(canvas.enlarged(factor))
}

pub(crate) fn cape(bytes: &[u8], factor: u32) -> Result<Rendered, String> {
    let texture = Texture::decode(bytes)?;
    let mut canvas = Canvas::new(CAPE_WIDTH, CAPE_HEIGHT);
    canvas.draw(
        &texture,
        patch(1, 1, CAPE_WIDTH, CAPE_HEIGHT),
        0,
        0,
        false,
        false,
    );
    Ok(canvas.enlarged(factor))
}
