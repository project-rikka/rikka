use crate::error::{Error, Result};

pub(crate) const WIDTH: u32 = 128;
pub(crate) const NARROW_WIDTH: u32 = 64;
pub(crate) const PROBE_LANES: u32 = 16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Extent {
    pub(crate) particles: u64,
    pub(crate) triangles: u64,
    pub(crate) body_vertices: u64,
    pub(crate) body_triangles: u64,
    pub(crate) binds: u64,
    pub(crate) boundaries: u64,
}

impl Extent {
    pub(crate) fn launchable(&self, widest: u32) -> Result<()> {
        let launches = [
            (
                "probe",
                self.particles * u64::from(PROBE_LANES),
                NARROW_WIDTH,
            ),
            ("targets", self.boundaries * self.particles, WIDTH),
            ("skin_body", self.boundaries * self.body_vertices, WIDTH),
            ("obstacles", self.body_triangles + self.triangles, WIDTH),
            ("wraps", self.triangles, NARROW_WIDTH),
            ("drape", self.binds, WIDTH),
        ];
        for (kernel, threads, width) in launches {
            let groups = threads.div_ceil(u64::from(width));
            if groups > u64::from(widest) {
                return Err(Error::DeviceLimit {
                    kernel,
                    groups,
                    limit: widest,
                });
            }
        }
        Ok(())
    }
}
