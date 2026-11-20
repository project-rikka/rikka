use std::ops::RangeInclusive;

use rikka_model_core::glam::Vec3;
use rikka_physics_tailor::{ClothSettings, Role};

use crate::mirror::Mirror;
use crate::surface::Surface;

const UNSPREAD: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tool {
    #[default]
    Free,
    Pinned,
    Skinned,
    Body,
    Spread,
}

impl Tool {
    pub const ALL: [Tool; 5] = [
        Tool::Free,
        Tool::Pinned,
        Tool::Skinned,
        Tool::Body,
        Tool::Spread,
    ];

    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Tool::Free => "free",
            Tool::Pinned => "pinned",
            Tool::Skinned => "skinned",
            Tool::Body => "body",
            Tool::Spread => "spread",
        }
    }

    #[must_use]
    pub fn role(self) -> Option<Role> {
        match self {
            Tool::Free => Some(Role::Free),
            Tool::Pinned => Some(Role::Pinned),
            Tool::Skinned => Some(Role::Skinned),
            Tool::Body => Some(Role::Body),
            Tool::Spread => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    pub tool: Tool,
    pub radius: f32,
    pub mirror: bool,
    pub spread: f32,
    pub strength: f32,
}

impl Brush {
    pub const RADII: RangeInclusive<f32> = 4.0..=160.0;
    pub const SPREADS: RangeInclusive<f32> = 0.0..=3.0;
    pub const STRENGTHS: RangeInclusive<f32> = 0.05..=1.0;

    pub fn dab(
        &self,
        cloth_settings: &mut ClothSettings,
        surface: &Surface,
        mirror: Option<&Mirror>,
        dab: Dab,
    ) -> usize {
        let affected: Vec<(usize, f32)> = surface.vertices_near(dab.centre, dab.radius).collect();
        let twins = mirror.filter(|_| self.mirror).map(|mirror| {
            affected
                .iter()
                .filter_map(|(index, weight)| Some((mirror.twin(*index)?, *weight)))
                .collect::<Vec<(usize, f32)>>()
        });
        let mut painted = 0;
        for (index, weight) in affected.iter().chain(twins.iter().flatten()) {
            let Some(key) = surface.keys().get(*index).copied() else {
                continue;
            };
            match (self.tool.role(), dab.erase) {
                (Some(_), true) => cloth_settings.paint_role(key, None),
                (Some(role), false) => cloth_settings.paint_role(key, Some(role)),
                (None, true) => cloth_settings.paint_spread(key, None),
                (None, false) => {
                    let spread = cloth_settings.paint_at(key).spread.unwrap_or(UNSPREAD);
                    let share = (self.strength * weight).clamp(0.0, 1.0);
                    cloth_settings.paint_spread(key, Some(spread + (self.spread - spread) * share));
                }
            }
            painted += 1;
        }
        painted
    }
}

impl Default for Brush {
    fn default() -> Brush {
        Brush {
            tool: Tool::Free,
            radius: 24.0,
            mirror: true,
            spread: 1.5,
            strength: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dab {
    pub centre: Vec3,
    pub radius: f32,
    pub erase: bool,
}
