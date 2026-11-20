mod cache;
mod cast;
mod contact;
mod epa;
mod gjk;
mod segment;
mod simplex;
mod toi;

pub use cache::SimplexCache;
pub use cast::{ShapeCastPairInput, shape_cast_pair};
pub(crate) use contact::closest_features;
pub use gjk::{DistanceInput, DistanceOutput, shape_distance};
pub use segment::{SegmentDistance, segment_distance};
pub(crate) use segment::{closest_on_segment, line_distance};
pub use toi::{Sweep, ToiInput, ToiOutput, ToiState, time_of_impact};
