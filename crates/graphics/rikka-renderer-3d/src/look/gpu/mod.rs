mod figure;
mod scene;
mod sky;
mod space;
mod trace;

pub(crate) use figure::GpuFigure;
pub(crate) use scene::GpuScene;
pub(crate) use sky::{GpuSky, sun_angular_radius};
pub(crate) use space::GpuSpace;
pub(crate) use trace::{GpuTrace, TRACED_LIGHTS, split};

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuLook {
    pub sky: GpuSky,
    pub space: GpuSpace,
    pub scene: GpuScene,
    pub trace: GpuTrace,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuEnvironment {
    pub irradiance: [[f32; 4]; 9],
}

const _: () = {
    assert!(
        size_of::<GpuLook>()
            == size_of::<GpuSky>()
                + size_of::<GpuSpace>()
                + size_of::<GpuScene>()
                + size_of::<GpuTrace>()
    );
    assert!(size_of::<GpuEnvironment>() == 144);
};
