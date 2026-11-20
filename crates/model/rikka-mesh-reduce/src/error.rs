use rikka_mesh_edit::VertId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("option {0} is out of range")]
    OutOfRange(&'static str),

    #[error("selection history has no selected vertex to merge at")]
    NoHistoryVertex,

    #[error("vertex {0:?} has an invalid weld target")]
    InvalidWeldTarget(VertId),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn check_range(value: f32, min: f32, max: f32, name: &'static str) -> Result<()> {
    match (min..=max).contains(&value) {
        true => Ok(()),
        false => Err(Error::OutOfRange(name)),
    }
}
