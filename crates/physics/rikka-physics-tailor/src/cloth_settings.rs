use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rikka_file_format::JsonFormat;
use rikka_model_core::NodeId;
use serde::{Deserialize, Serialize};

use crate::analysis::AnalyzedPiece;
use crate::asset::{PieceKey, PieceKind};
use crate::error::Result;
use crate::key::VertexKey;
use crate::preset::Preset;
use crate::style::Style;
use crate::tuning::ClothTuning;

const SAMPLE: usize = 32;
const OVERLAP: f32 = 0.5;
const FORMAT: JsonFormat = JsonFormat::new("rikka-cloth");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Body,
    Skinned,
    Free,
    Pinned,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Paint {
    pub role: Option<Role>,
    pub spread: Option<f32>,
}

impl Paint {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.role.is_none() && self.spread.is_none()
    }

    fn sanitized(self) -> Paint {
        Paint {
            role: self.role,
            spread: self
                .spread
                .filter(|spread| spread.is_finite() && *spread >= 0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PieceSettings {
    pub key: PieceKey,
    pub enabled: bool,
    pub preset: Option<Preset>,
    pub kind: Option<PieceKind>,
    pub layer: Option<u8>,
    pub tuning: ClothTuning,
    pub style: Option<Style>,
    sample: Vec<VertexKey>,
}

impl PieceSettings {
    #[must_use]
    pub fn new(key: PieceKey) -> PieceSettings {
        PieceSettings {
            key,
            enabled: true,
            preset: None,
            kind: None,
            layer: None,
            tuning: ClothTuning::NEUTRAL,
            style: None,
            sample: Vec::new(),
        }
    }

    #[must_use]
    pub fn resolved_style(&self, preset: Preset) -> Style {
        match &self.style {
            Some(style) => style.clone(),
            None => self.tuning.apply(&preset.style()),
        }
    }

    fn cuts(&self) -> bool {
        !self.enabled || self.preset.is_some() || self.kind.is_some() || self.layer.is_some()
    }

    fn same_topology(&self, other: &PieceSettings) -> bool {
        self.key == other.key
            && self.enabled == other.enabled
            && self.preset == other.preset
            && self.kind == other.kind
            && self.layer == other.layer
            && self.sample == other.sample
    }

    fn overlap(&self, members: &[VertexKey]) -> f32 {
        if self.sample.is_empty() {
            return 0.0;
        }
        let matched = self
            .sample
            .iter()
            .filter(|key| members.binary_search(key).is_ok())
            .count();
        matched as f32 / self.sample.len() as f32
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClothSettings {
    paint: BTreeMap<VertexKey, Paint>,
    pieces: Vec<PieceSettings>,
    tuning: ClothTuning,
}

#[derive(Serialize, Deserialize)]
struct PaintEntry {
    node: u32,
    vertex: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<Role>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spread: Option<f32>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsFile {
    paint: Vec<PaintEntry>,
    pieces: Vec<PieceSettings>,
    tuning: ClothTuning,
}

impl ClothSettings {
    pub const EXTENSION: &'static str = "rcloth";

    #[must_use]
    pub fn new() -> ClothSettings {
        ClothSettings::default()
    }

    #[must_use]
    pub fn sidecar_path(model_path: &Path) -> PathBuf {
        let mut name = model_path
            .file_name()
            .map_or_else(OsString::new, OsString::from);
        name.push(".");
        name.push(ClothSettings::EXTENSION);
        model_path.with_file_name(name)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<ClothSettings> {
        Ok(ClothSettings::restored(FORMAT.load(path)?))
    }

    pub fn load_for_model(model_path: &Path) -> Result<ClothSettings> {
        match FORMAT.load(ClothSettings::sidecar_path(model_path)) {
            Ok(file) => Ok(ClothSettings::restored(file)),
            Err(rikka_file_format::Error::Read { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(ClothSettings::new())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(FORMAT.save(path, &self.stored())?)
    }

    pub fn save_for_model(&self, model_path: &Path) -> Result<()> {
        self.save(ClothSettings::sidecar_path(model_path))
    }

    fn stored(&self) -> SettingsFile {
        SettingsFile {
            paint: self
                .paint
                .iter()
                .map(|(key, paint)| PaintEntry {
                    node: key.node.0,
                    vertex: key.vertex,
                    role: paint.role,
                    spread: paint.spread,
                })
                .collect(),
            pieces: self.pieces.clone(),
            tuning: self.tuning,
        }
    }

    fn restored(file: SettingsFile) -> ClothSettings {
        let mut cloth_settings = ClothSettings::new();
        for entry in file.paint {
            cloth_settings.paint(
                VertexKey::new(NodeId(entry.node), entry.vertex),
                Paint {
                    role: entry.role,
                    spread: entry.spread,
                },
            );
        }
        for mut settings in file.pieces {
            settings.sample.sort_unstable();
            settings.sample.dedup();
            settings.sample.truncate(SAMPLE);
            cloth_settings.pieces.push(settings);
        }
        cloth_settings.tuning = file.tuning.clamped();
        cloth_settings
    }

    pub fn paint(&mut self, vertex: VertexKey, paint: Paint) {
        let paint = paint.sanitized();
        if paint.is_empty() {
            self.paint.remove(&vertex)
        } else {
            self.paint.insert(vertex, paint)
        };
    }

    pub fn paint_role(&mut self, vertex: VertexKey, role: Option<Role>) {
        let paint = Paint {
            role,
            ..self.paint_at(vertex)
        };
        self.paint(vertex, paint);
    }

    pub fn paint_spread(&mut self, vertex: VertexKey, spread: Option<f32>) {
        let paint = Paint {
            spread,
            ..self.paint_at(vertex)
        };
        self.paint(vertex, paint);
    }

    #[must_use]
    pub fn paint_at(&self, vertex: VertexKey) -> Paint {
        self.paint.get(&vertex).copied().unwrap_or_default()
    }

    pub fn paint_entries(&self) -> impl Iterator<Item = (VertexKey, Paint)> + '_ {
        self.paint.iter().map(|(key, paint)| (*key, *paint))
    }

    #[must_use]
    pub fn same_topology(&self, other: &ClothSettings) -> bool {
        self.paint == other.paint
            && self.cutting().count() == other.cutting().count()
            && self
                .cutting()
                .zip(other.cutting())
                .all(|(one, two)| one.same_topology(two))
    }

    fn cutting(&self) -> impl Iterator<Item = &PieceSettings> {
        self.pieces.iter().filter(|settings| settings.cuts())
    }

    pub fn clear_paint(&mut self) {
        self.paint.clear();
    }

    #[must_use]
    pub fn tuning(&self) -> ClothTuning {
        self.tuning
    }

    pub fn set_tuning(&mut self, tuning: ClothTuning) {
        self.tuning = tuning.clamped();
    }

    #[must_use]
    pub fn pieces(&self) -> &[PieceSettings] {
        &self.pieces
    }

    #[must_use]
    pub fn settings(&self, piece: &AnalyzedPiece) -> Option<&PieceSettings> {
        self.matched(std::slice::from_ref(piece))[0].map(|index| &self.pieces[index])
    }

    pub fn settings_mut(&mut self, piece: &AnalyzedPiece) -> &mut PieceSettings {
        let index = match self.matched(std::slice::from_ref(piece))[0] {
            Some(index) => index,
            None => {
                self.pieces.push(PieceSettings::new(piece.key));
                self.pieces.len() - 1
            }
        };
        let settings = &mut self.pieces[index];
        settings.key = piece.key;
        settings.sample = sample(piece.members());
        settings
    }

    pub fn sync_pieces(&mut self, pieces: &[AnalyzedPiece]) {
        for (piece, index) in pieces.iter().zip(self.matched(pieces)) {
            if let Some(index) = index {
                self.pieces[index].key = piece.key;
                self.pieces[index].sample = sample(piece.members());
            }
        }
    }

    pub(crate) fn matched(&self, pieces: &[AnalyzedPiece]) -> Vec<Option<usize>> {
        let mut found = vec![None; pieces.len()];
        let mut taken = vec![false; self.pieces.len()];
        for (piece, slot) in pieces.iter().zip(&mut found) {
            if let Some(index) = self
                .pieces
                .iter()
                .enumerate()
                .position(|(index, settings)| !taken[index] && settings.key == piece.key)
            {
                taken[index] = true;
                *slot = Some(index);
            }
        }
        let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
        for (at, piece) in pieces.iter().enumerate() {
            if found[at].is_some() {
                continue;
            }
            for (index, settings) in self.pieces.iter().enumerate() {
                let overlap = settings.overlap(piece.members());
                if !taken[index] && overlap >= OVERLAP {
                    pairs.push((overlap, at, index));
                }
            }
        }
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for (_, at, index) in pairs {
            if found[at].is_none() && !taken[index] {
                taken[index] = true;
                found[at] = Some(index);
            }
        }
        found
    }
}

fn sample(members: &[VertexKey]) -> Vec<VertexKey> {
    if members.len() <= SAMPLE {
        return members.to_vec();
    }
    let mut sample: Vec<VertexKey> = (0..SAMPLE)
        .map(|step| members[step * (members.len() - 1) / (SAMPLE - 1)])
        .collect();
    sample.dedup();
    sample
}
