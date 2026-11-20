use crate::error::{Error, Result};

pub(crate) const OUTLINE_COLORS: usize = 15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionOutline {
    pub pixel_size: f32,
    pub thick: bool,
    pub occluded_alpha: f32,
    pub smooth: bool,
}

impl Default for SelectionOutline {
    fn default() -> Self {
        Self {
            pixel_size: 1.0,
            thick: false,
            occluded_alpha: 0.35,
            smooth: true,
        }
    }
}

impl SelectionOutline {
    pub(crate) fn expands(&self) -> bool {
        self.thick || self.pixel_size > 1.0
    }

    pub(crate) fn line_kernel(&self) -> f32 {
        self.pixel_size * 0.5 - 0.5
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct OutlinePalette {
    colors: Vec<[u8; 4]>,
}

impl OutlinePalette {
    pub(crate) fn clear(&mut self) {
        self.colors.clear();
    }

    pub(crate) fn code(&mut self, color: [u8; 4]) -> Result<u32> {
        let index = match self.colors.iter().position(|known| *known == color) {
            Some(index) => index,
            None if self.colors.len() < OUTLINE_COLORS => {
                self.colors.push(color);
                self.colors.len() - 1
            }
            None => {
                return Err(Error::Invalid(format!(
                    "a frame outlines draws in more than {OUTLINE_COLORS} colors"
                )));
            }
        };
        Ok(index as u32 + 1)
    }

    pub(crate) fn packed(&self) -> [u32; OUTLINE_COLORS + 1] {
        let mut packed = [0; OUTLINE_COLORS + 1];
        for (slot, color) in packed[1..].iter_mut().zip(&self.colors) {
            *slot = u32::from_le_bytes(*color);
        }
        packed
    }
}
