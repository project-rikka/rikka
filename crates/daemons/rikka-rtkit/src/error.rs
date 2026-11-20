use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the kernel refused ({kernel}) and RealtimeKit failed: {realtime_kit}")]
    Refused {
        kernel: io::Error,
        realtime_kit: zbus::Error,
    },
    #[error(
        "the kernel refused ({kernel}) and the budget RealtimeKit requires was not set: {budget}"
    )]
    Unbudgeted {
        kernel: io::Error,
        budget: io::Error,
    },
}
