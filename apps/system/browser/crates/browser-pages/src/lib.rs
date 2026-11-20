mod certificate;
mod failure;
mod hosts;
mod icons;
mod memory;
mod origin;
mod route;
mod visits;

pub use certificate::Certificate;
pub use failure::{ErrorAction, FailureKind};
pub use hosts::{covers, host_key, registrable_site, site_of, web_host, web_site};
pub use icons::{IconCandidate, best_icon, data_url_payload};
pub use memory::{TabActivity, discardable_tabs, is_exempt, is_pressed};
pub use origin::Origin;
pub use route::{Internal, SCHEME, Section, VIEW_SOURCE};
pub use visits::{Arrival, Cause, VisitTracker, Visited, is_recorded_scheme};
