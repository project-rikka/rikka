mod catalogue;
mod domain;
mod features;
mod logicals;
mod query;
mod scoring;
mod status;

pub use catalogue::{CityNames, ServerCounts, StreamingService, StreamingServices, UserLocation};
pub use domain::{ConnectingDomain, ServerEntry, ServerEntryInfo};
pub use features::{ServerFeatures, Tier};
pub use logicals::{
    LogicalServer, Logicals, LogicalsMetadata, ServerLocation, ServerStatusReference,
};
pub use query::ServerQuery;
pub use scoring::Scoring;
pub use status::{BinaryStatus, ServerStatus};
