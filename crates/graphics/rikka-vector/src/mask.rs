use crate::pixmap::Pixmap;

const RED: u32 = 54;
const GREEN: u32 = 183;
const BLUE: u32 = 19;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mask {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Mask {
    #[must_use]
    pub fn from_alpha(pixmap: &Pixmap) -> Mask {
        Mask {
            width: pixmap.width(),
            height: pixmap.height(),
            data: pixmap
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel[3])
                .collect(),
        }
    }

    #[must_use]
    pub fn from_luminance(pixmap: &Pixmap) -> Mask {
        Mask {
            width: pixmap.width(),
            height: pixmap.height(),
            data: pixmap
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|&[red, green, blue, _]| {
                    let weighted =
                        RED * u32::from(red) + GREEN * u32::from(green) + BLUE * u32::from(blue);
                    ((weighted + 128) >> 8) as u8
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn from_linear_luminance(pixmap: &Pixmap) -> Mask {
        let mut mask = Mask::from_alpha(pixmap);
        for (value, pixel) in mask.data.iter_mut().zip(pixmap.data().as_chunks::<4>().0) {
            let alpha = f32::from(pixel[3]) / 255.0;
            if alpha <= 0.0 {
                continue;
            }
            let linear = |channel: u8| {
                rikka_types::srgb::to_linear((f32::from(channel) / 255.0 / alpha).min(1.0))
            };
            let luminance =
                0.2125 * linear(pixel[0]) + 0.7154 * linear(pixel[1]) + 0.0721 * linear(pixel[2]);
            *value = (luminance * alpha * 255.0 + 0.5) as u8;
        }
        mask
    }

    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}
