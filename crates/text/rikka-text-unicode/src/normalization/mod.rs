mod decomposition;
mod hangul;
mod marks;
mod normalize;

pub use decomposition::{Decomposition, canonical_decomposition, compose, decompose};
pub use normalize::{
    is_nfc, is_nfd, nfc, nfc_into, nfd, nfd_into, nfkc, nfkc_into, nfkd, nfkd_into,
};
