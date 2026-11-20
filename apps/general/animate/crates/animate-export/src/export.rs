use animate_cast::Resolution;

use crate::encoding::Encoding;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Export {
    pub resolution: Resolution,
    pub encoding: Encoding,
    pub first: u32,
    pub last: u32,
}
