mod code;
mod context;
mod enums;
mod error;
mod generate;
mod handle;
mod incoming;
mod interface;
mod model;
mod names;
mod parse;
mod proxy;
mod side;
mod types;

use std::path::Path;

pub use crate::error::{Error, Result};
use crate::generate::Emission;
use crate::model::Protocol;

pub fn shared(protocols: &[impl AsRef<Path>], runtime: &str) -> Result<String> {
    generate::emit(&parse(protocols)?, runtime, Emission::Shared)
}

pub fn server(protocols: &[impl AsRef<Path>], runtime: &str, shared: &str) -> Result<String> {
    generate::emit(&parse(protocols)?, runtime, Emission::Server { shared })
}

pub fn client(protocols: &[impl AsRef<Path>], runtime: &str, shared: &str) -> Result<String> {
    generate::emit(&parse(protocols)?, runtime, Emission::Client { shared })
}

fn parse(protocols: &[impl AsRef<Path>]) -> Result<Vec<Protocol>> {
    protocols
        .iter()
        .map(|path| parse::protocol(path.as_ref()))
        .collect()
}
