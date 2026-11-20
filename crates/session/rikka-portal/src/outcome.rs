use std::io::{Read, Write};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::wire;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "outcome")]
pub enum Outcome<A> {
    Chosen(A),
    Cancelled,
}

impl<A: DeserializeOwned> Outcome<A> {
    pub fn read(reader: impl Read) -> Result<Outcome<A>> {
        wire::read(reader)
    }
}

impl<A: Serialize> Outcome<A> {
    pub fn write(&self, writer: impl Write) -> Result<()> {
        wire::write(self, writer)
    }
}
