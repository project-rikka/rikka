use rikka_gpu::GpuBuilder;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Device {
    pub(crate) three: bool,
    pub(crate) video: bool,
    pub(crate) compute: bool,
    pub(crate) background: bool,
}

impl Device {
    pub(crate) fn ask(self, builder: GpuBuilder) -> GpuBuilder {
        let builder = match self.video {
            true => builder.video_decode(),
            false => builder,
        };
        let builder = match (self.compute, self.background) {
            (_, true) => builder.background_compute(),
            (true, false) => builder.compute_support(),
            (false, false) => builder,
        };
        match self.three {
            true => builder.with_3d(),
            false => builder,
        }
    }
}
