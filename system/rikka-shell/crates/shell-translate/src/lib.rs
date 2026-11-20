mod choices;
mod error;
mod feature;
mod language;
mod model;
mod picker;
mod profile;
mod prompts;
mod service;
mod shared;
mod side;
mod translator;
mod view;
mod window;

pub use feature::Translator;
use rikka_seam::Request;
use shell_capture::Snip;

use crate::service::{CAPABILITY, PICTURE};
#[must_use]
pub fn snip() -> Snip {
    Snip::with_request(Request::new(CAPABILITY, PICTURE))
}
