mod dff;
mod dsf;
mod info;
mod layout;
mod reader;

use std::io::{Read, Seek};

pub use info::DsdInfo;
pub use reader::DsdReader;

use layout::Header;

fn header(source: &mut (impl Read + Seek)) -> Option<Header> {
    dsf::header(source).or_else(|| dff::header(source))
}
