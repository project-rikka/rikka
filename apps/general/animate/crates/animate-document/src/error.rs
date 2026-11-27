#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] rikka_file_format::Error),
    #[error(transparent)]
    Take(#[from] animate_motion::Error),
    #[error(transparent)]
    Look(#[from] animate_look::Error),
    #[error(transparent)]
    Fluid(#[from] animate_fluid::Error),
    #[error(transparent)]
    Wind(#[from] animate_wind::Error),
    #[error(transparent)]
    Prop(#[from] animate_prop::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
