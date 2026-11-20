use std::ffi::OsStr;

use nvml_wrapper::Nvml;
use nvml_wrapper::error::NvmlError;

const SONAME: &str = "libnvidia-ml.so.1";

const NIXOS_DRIVER: &str = "/run/opengl-driver/lib/libnvidia-ml.so.1";

pub fn load() -> Result<Nvml, NvmlError> {
    Nvml::builder()
        .lib_path(OsStr::new(SONAME))
        .init()
        .or_else(|missing| {
            Nvml::builder()
                .lib_path(OsStr::new(NIXOS_DRIVER))
                .init()
                .map_err(|_| missing)
        })
}
