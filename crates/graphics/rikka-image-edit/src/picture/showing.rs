use std::sync::Arc;

use rikka_gpu::{ComputedFrame, ImageFormat};

use crate::color::{ConversionKind, Space};
use crate::error::Result;
use crate::picture::shown::{ConvertPush, shown_parts};
use crate::picture::{Depth, Picture};
use crate::plane::Plane;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

#[derive(Clone)]
pub struct Shown {
    frame: ComputedFrame,
    plane: Plane,
    tile_ids: Arc<[u64]>,
    space: Arc<Space>,
    zero_copy: bool,
}

impl Shown {
    #[must_use]
    pub fn frame(&self) -> &ComputedFrame {
        &self.frame
    }
}

impl std::fmt::Debug for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shown")
            .field("extent", &self.plane.extent())
            .field("tiles", &self.tile_ids.len())
            .finish()
    }
}

impl Picture {
    pub fn show(&self, earlier: Option<&Shown>) -> Result<Shown> {
        let frame = self.frame();
        let source = &frame.plane;
        let mut work = Work::new(self.imaging());
        let display = frame.space.display(&work)?;
        let zero_copy = frame.depth == Depth::Eight
            && !frame.has_alpha
            && source.format() == ImageFormat::Rgba8
            && display.kind == ConversionKind::Identity;
        let tile_ids: Arc<[u64]> = source.tiles().iter().map(|tile| tile.pixels_id()).collect();
        let earlier = earlier.filter(|earlier| {
            earlier.zero_copy == zero_copy
                && earlier.plane.extent() == source.extent()
                && earlier.tile_ids.len() == tile_ids.len()
                && Arc::ptr_eq(&earlier.space, &frame.space)
        });
        let changed_tiles: Vec<bool> = match earlier {
            Some(earlier) => earlier
                .tile_ids
                .iter()
                .zip(tile_ids.iter())
                .map(|(then, now)| then != now)
                .collect(),
            None => vec![true; tile_ids.len()],
        };
        if let Some(earlier) = earlier
            && !changed_tiles.contains(&true)
        {
            return Ok(earlier.clone());
        }
        let out = match (zero_copy, earlier) {
            (true, _) => source.clone(),
            (false, Some(earlier)) => {
                work.derived(&earlier.plane, ImageFormat::Rgba8, |index, _| {
                    changed_tiles[index]
                })?
            }
            (false, None) => work.plane(source.extent(), ImageFormat::Rgba8)?,
        };
        if !zero_copy {
            let push = ConvertPush {
                head: PushHeader::new(source, &out)
                    .mode(display.kind as u32)
                    .clamped(true),
                data: display.data.address(),
                cube: display.cube,
                premultiply: 1,
                from: [0, 0],
            };
            let changed = out
                .areas()
                .zip(&changed_tiles)
                .filter(|(_, changed_tiles)| **changed_tiles);
            for (area, _) in changed {
                let io = Io::new(source, &out)
                    .read_buffer(&display.data)
                    .limit_writes(area);
                work.pass_at(Shader::Convert, push, io, area, 1.0)?;
            }
        }
        let ticket = work.finish()?;
        Ok(Shown {
            frame: shown_parts(&out, &ticket)?,
            plane: out,
            tile_ids,
            space: Arc::clone(&frame.space),
            zero_copy,
        })
    }
}
