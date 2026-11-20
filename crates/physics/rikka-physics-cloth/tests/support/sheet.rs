use std::sync::Arc;

use glam::Vec3;
use rikka_model_core::{Instance, Model};
use rikka_physics_tailor::{ClothAsset, PieceKind};

use super::scene::sheet_asset;

pub(crate) struct Sheet {
    pub(crate) model: Arc<Model>,
    pub(crate) asset: Arc<ClothAsset>,
    pub(crate) columns: u32,
    pub(crate) rows: u32,
    pub(crate) spacing: f32,
}

impl Sheet {
    pub(crate) fn new(columns: u32, rows: u32, spacing: f32, kind: PieceKind) -> Sheet {
        let (model, asset) = sheet_asset(columns, rows, spacing, kind);
        Sheet {
            model,
            asset,
            columns,
            rows,
            spacing,
        }
    }

    pub(crate) fn instance(&self) -> Instance {
        Instance::new(Arc::clone(&self.model))
    }

    pub(crate) fn rest(&self) -> Vec<Vec3> {
        self.asset.pieces[0]
            .particles
            .iter()
            .map(|particle| particle.rest)
            .collect()
    }
}
