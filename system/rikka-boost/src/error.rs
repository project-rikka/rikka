use rikka_boost::service::SERVICE;
use rikka_nvml::NvmlError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("failed to load NVML: {0}")]
    Nvml(#[source] NvmlError),
    #[error("no NVIDIA GPU with lockable clocks")]
    NoGpu,
    #[error("failed to connect to the system bus: {0}")]
    Bus(#[source] zbus::Error),
    #[error("failed to acquire {SERVICE} on the system bus: {0}")]
    NameRequest(#[source] zbus::Error),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
