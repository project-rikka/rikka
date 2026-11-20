use rikka_file_format::{Codec, Reader, Writer};
use rikka_physics_geometry::Tolerance;
use rikka_physics_math::Simd;

use crate::settings::Settings;
use crate::{CharacterDef, Error, Result};

impl Codec for Settings {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.gravity);
        out.put(&self.tick_rate);
        out.put(&self.substeps);
        out.put(&self.max_ticks);
        out.put(&self.tolerance.length_scale);
        out.put(&(self.simd.lanes() as u8));
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        let gravity = input.get()?;
        let tick_rate = input.get()?;
        let substeps = input.get()?;
        let max_ticks = input.get()?;
        let length_scale = input.get()?;
        let lanes = input.get::<u8>()? as usize;
        let simd = Simd::with_lanes(lanes)?;
        if tick_rate == 0 || substeps == 0 || max_ticks == 0 {
            return Err(Error::MalformedRecording("invalid world settings"));
        }
        Ok(Self {
            gravity,
            tick_rate,
            substeps,
            max_ticks,
            tolerance: Tolerance { length_scale },
            simd,
        })
    }
}

impl Codec for CharacterDef {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.mover);
        out.put(&self.position);
        out.put(&self.up);
        out.put(&self.max_slope);
        out.put(&self.step_height);
        out.put(&self.filter);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            mover: input.get()?,
            position: input.get()?,
            up: input.get()?,
            max_slope: input.get()?,
            step_height: input.get()?,
            filter: input.get()?,
        })
    }
}
