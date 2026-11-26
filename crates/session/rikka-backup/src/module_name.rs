use rikka_nixos_layout::Module;
use serde::{Deserialize, Deserializer, Serializer};

pub(crate) fn serialize<S: Serializer>(module: &Module, into: S) -> Result<S::Ok, S::Error> {
    into.serialize_str(module.name())
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Module, D::Error> {
    let name = String::deserialize(from)?;
    Module::from_name(&name)
        .ok_or_else(|| serde::de::Error::custom(format!("`{name}` is not a Rikka module")))
}
