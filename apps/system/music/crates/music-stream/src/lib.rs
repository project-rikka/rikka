mod fetcher;
mod opening;
mod origin;
mod paging;
mod plan;
mod scan;
mod spool;
mod streamer;
mod web;

pub use fetcher::Fetcher;
pub use origin::{Located, Origin};
pub use paging::OffsetPaging;
pub use plan::Plan;
pub use scan::{Matches, Scan, is_word};
pub use spool::{Spool, Stored};
pub use streamer::Streamer;
pub use web::{answer, answer_bytes, answer_length, answer_text, api_client};
