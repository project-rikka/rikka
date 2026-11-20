use half::f16;
use rikka_color_lookup::ColorLookup;

use crate::error::Result;
use crate::texture::{Rows, Staged, TexelRect, TextureId, Uploads};

const HALF_RGBA: u32 = u32::from_le_bytes(*b"AB4H");

impl Uploads {
    pub fn create_color_lookup(&self, lookup: &ColorLookup) -> Result<TextureId> {
        let size = lookup.size();
        let extent = [(size * size) as u32, size as u32];
        let mut texels = Vec::with_capacity(size * size * size * 8);
        for green in 0..size {
            for blue in 0..size {
                for red in 0..size {
                    let [r, g, b] = lookup.entry(red, green, blue);
                    for channel in [r, g, b, 1.0] {
                        texels.extend(f16::from_f32(channel).to_le_bytes());
                    }
                }
            }
        }
        let texture = self.create_drm(HALF_RGBA, extent[0], extent[1])?;
        let sent = self
            .stage(
                texture,
                Rows::new(&texels, extent[0] as usize * 8),
                &[TexelRect::new([0, 0], extent)],
            )
            .and_then(Staged::send);
        match sent {
            Ok(()) => Ok(texture),
            Err(error) => {
                self.destroy(texture);
                Err(error)
            }
        }
    }
}
