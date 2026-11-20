mod dope;
mod layout;
mod playhead;
mod readout;
mod state;
mod wording;

pub use dope::{Dope, Names};
pub use playhead::playhead;
pub use readout::{Readout, WIDTH as READOUT_WIDTH};
pub use state::Timeline;

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
