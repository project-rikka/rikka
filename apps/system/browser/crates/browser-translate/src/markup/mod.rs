mod decode;
pub(crate) mod dialect;
mod encode;
mod normalize;
mod plan;

pub use dialect::Dialect;
pub(crate) use encode::{Encoded, encode};

use crate::unit::Output;

impl Encoded {
    pub(crate) fn decode(&self, html: &str, dialect: &Dialect) -> Output {
        normalize::normalize(&self.plan, decode::decode(html, &self.plan, dialect))
    }
}
