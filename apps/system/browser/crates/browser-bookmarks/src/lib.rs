mod error;
mod layout;
mod merge;
mod mirror;
mod plan;
mod remote;
mod shadow;
mod side;
mod sides;

pub use error::Error;
pub use layout::{FAVORITES, RIKKA, SPACES};
pub use mirror::{Mirror, MirrorFavorite, MirrorNode, MirrorSpace};
pub use plan::{Pending, Plan};
pub use remote::Remote;
pub use side::LocalSide;
