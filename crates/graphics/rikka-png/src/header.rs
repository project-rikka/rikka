use crate::error::{Error, Result};
use crate::layout::{Layout, Limits};

const NON_INTERLACED: [(usize, usize, usize, usize); 1] = [(0, 0, 1, 1)];
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Color {
    Gray,
    Rgb,
    Indexed,
    GrayAlpha,
    Rgba,
}

impl Color {
    fn parse(raw: u8, depth: u8) -> Option<Self> {
        let color = match raw {
            0 => Self::Gray,
            2 => Self::Rgb,
            3 => Self::Indexed,
            4 => Self::GrayAlpha,
            6 => Self::Rgba,
            _ => return None,
        };
        let valid = match color {
            Self::Gray => matches!(depth, 1 | 2 | 4 | 8 | 16),
            Self::Indexed => matches!(depth, 1 | 2 | 4 | 8),
            Self::Rgb | Self::GrayAlpha | Self::Rgba => matches!(depth, 8 | 16),
        };
        valid.then_some(color)
    }

    pub(crate) const fn channels(self) -> usize {
        match self {
            Self::Gray | Self::Indexed => 1,
            Self::GrayAlpha => 2,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }

    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Gray => 0,
            Self::Rgb => 2,
            Self::Indexed => 3,
            Self::GrayAlpha => 4,
            Self::Rgba => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Header {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) depth: u8,
    pub(crate) color: Color,
    pub(crate) interlaced: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Pass {
    pub(crate) x0: usize,
    pub(crate) y0: usize,
    pub(crate) dx: usize,
    pub(crate) dy: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl Header {
    pub(crate) const LEN: usize = 13;

    pub(crate) fn parse(body: &[u8]) -> Result<Self> {
        let body: &[u8; Self::LEN] = body.try_into().map_err(|_| Error::Header)?;
        let width = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
        let height = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
        let depth = body[8];
        let color = Color::parse(body[9], depth).ok_or(Error::Header)?;
        let valid = width > 0
            && height > 0
            && width <= i32::MAX as u32
            && height <= i32::MAX as u32
            && body[10] == 0
            && body[11] == 0
            && body[12] <= 1;
        if !valid {
            return Err(Error::Header);
        }
        Ok(Self {
            width: width as usize,
            height: height as usize,
            depth,
            color,
            interlaced: body[12] == 1,
        })
    }

    pub(crate) fn encode(&self) -> [u8; Self::LEN] {
        let mut body = [0; Self::LEN];
        body[0..4].copy_from_slice(&(self.width as u32).to_be_bytes());
        body[4..8].copy_from_slice(&(self.height as u32).to_be_bytes());
        body[8] = self.depth;
        body[9] = self.color.code();
        body[12] = u8::from(self.interlaced);
        body
    }

    pub(crate) fn with_size(self, width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            ..self
        }
    }

    pub(crate) fn layout(&self, keyed: bool) -> Layout {
        let channels = match self.color {
            Color::Gray => 1 + usize::from(keyed),
            Color::GrayAlpha => 2,
            Color::Rgb | Color::Indexed => 3 + usize::from(keyed),
            Color::Rgba => 4,
        };
        Layout::new(channels, self.depth == 16)
    }

    pub(crate) fn admit(&self, layout: Layout, limits: &Limits) -> Result<usize> {
        let exceeded = Error::LimitsExceeded {
            width: self.width as u32,
            height: self.height as u32,
        };
        let bytes = (self.width as u64)
            .checked_mul(self.height as u64)
            .and_then(|pixels| pixels.checked_mul(layout.bytes_per_pixel() as u64))
            .filter(|&bytes| bytes <= limits.max_bytes && bytes <= isize::MAX as u64 / 2);
        match bytes {
            Some(bytes)
                if self.width as u64 <= u64::from(limits.max_width)
                    && self.height as u64 <= u64::from(limits.max_height) =>
            {
                Ok(bytes as usize)
            }
            _ => Err(exceeded),
        }
    }

    pub(crate) fn bits_per_pixel(&self) -> usize {
        self.color.channels() * usize::from(self.depth)
    }

    pub(crate) fn row_bytes(&self, width: usize) -> usize {
        (width * self.bits_per_pixel()).div_ceil(8)
    }

    pub(crate) fn filter_stride(&self) -> usize {
        (self.bits_per_pixel() / 8).max(1)
    }

    pub(crate) fn passes(&self) -> impl Iterator<Item = Pass> + '_ {
        let passes: &[(usize, usize, usize, usize)] = if self.interlaced {
            &ADAM7
        } else {
            &NON_INTERLACED
        };
        passes.iter().filter_map(|&(x0, y0, dx, dy)| {
            let width = (self.width + dx - 1 - x0) / dx;
            let height = (self.height + dy - 1 - y0) / dy;
            (self.width > x0 && self.height > y0 && width > 0 && height > 0).then_some(Pass {
                x0,
                y0,
                dx,
                dy,
                width,
                height,
            })
        })
    }

    pub(crate) fn raw_len(&self) -> usize {
        self.passes()
            .map(|pass| pass.height * (1 + self.row_bytes(pass.width)))
            .sum()
    }
}
