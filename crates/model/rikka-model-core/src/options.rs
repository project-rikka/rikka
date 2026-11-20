use crate::coordinates::CoordinateSystem;
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Limit {
    FileBytes,
    ResourceBytes,
    TotalBytes,
    Vertices,
    Indices,
    Keyframes,
}

impl Limit {
    pub fn check(self, actual: u64, allowed: u64) -> Result<()> {
        match actual <= allowed {
            true => Ok(()),
            false => Err(Error::LimitsExceeded {
                limit: self,
                actual,
                allowed,
            }),
        }
    }
}

impl std::fmt::Display for Limit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::FileBytes => "file size",
            Self::ResourceBytes => "resource size",
            Self::TotalBytes => "total size",
            Self::Vertices => "vertex count",
            Self::Indices => "index count",
            Self::Keyframes => "keyframe count",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_file_bytes: u64,
    pub max_resource_bytes: u64,
    pub max_total_bytes: u64,
    pub max_vertices: usize,
    pub max_indices: usize,
    pub max_keyframes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 512 * 1024 * 1024,
            max_resource_bytes: 256 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
            max_vertices: 16_000_000,
            max_indices: 64_000_000,
            max_keyframes: 32_000_000,
        }
    }
}

impl Limits {
    #[must_use]
    pub fn allowed(&self, limit: Limit) -> u64 {
        match limit {
            Limit::FileBytes => self.max_file_bytes,
            Limit::ResourceBytes => self.max_resource_bytes,
            Limit::TotalBytes => self.max_total_bytes,
            Limit::Vertices => self.max_vertices as u64,
            Limit::Indices => self.max_indices as u64,
            Limit::Keyframes => self.max_keyframes as u64,
        }
    }

    pub fn check(&self, limit: Limit, actual: u64) -> Result<()> {
        limit.check(actual, self.allowed(limit))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct ImportOptions {
    pub coordinates: CoordinateSystem,
    pub scale: f32,
    pub normalize_units: bool,
    pub normalize_weights: bool,
    pub limits: Limits,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            coordinates: CoordinateSystem::CANONICAL,
            scale: 1.0,
            normalize_units: true,
            normalize_weights: true,
            limits: Limits::default(),
        }
    }
}

impl ImportOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn coordinates(mut self, coordinates: CoordinateSystem) -> Self {
        self.coordinates = coordinates;
        self
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn normalize_units(mut self, normalize: bool) -> Self {
        self.normalize_units = normalize;
        self
    }

    pub fn normalize_weights(mut self, normalize: bool) -> Self {
        self.normalize_weights = normalize;
        self
    }

    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct ExportOptions {
    pub coordinates: CoordinateSystem,
    pub unit_scale: f32,
    pub embed_resources: bool,
    pub limits: Limits,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            coordinates: CoordinateSystem::CANONICAL,
            unit_scale: 1.0,
            embed_resources: true,
            limits: Limits::default(),
        }
    }
}

impl ExportOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn coordinates(mut self, coordinates: CoordinateSystem) -> Self {
        self.coordinates = coordinates;
        self
    }

    pub fn unit_scale(mut self, metres_per_unit: f32) -> Self {
        self.unit_scale = metres_per_unit;
        self
    }

    pub fn embed_resources(mut self, embed: bool) -> Self {
        self.embed_resources = embed;
        self
    }

    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}
