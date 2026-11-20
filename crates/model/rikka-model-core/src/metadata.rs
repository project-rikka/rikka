use std::path::PathBuf;

use crate::coordinates::CoordinateSystem;

#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    pub importer: &'static str,
    pub generator: Option<String>,
    pub source: Option<PathBuf>,
    pub coordinates: CoordinateSystem,
    pub unit_scale: f32,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            importer: "",
            generator: None,
            source: None,
            coordinates: CoordinateSystem::CANONICAL,
            unit_scale: 1.0,
        }
    }
}
