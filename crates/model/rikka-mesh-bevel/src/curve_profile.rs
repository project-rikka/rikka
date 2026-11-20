use glam::Vec2;

use crate::curve_sampling::{even_length_samples, path_samples};
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HandleType {
    Free,
    #[default]
    Auto,
    Vector,
    Align,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePoint {
    pub position: Vec2,
    pub handle_types: [HandleType; 2],
    pub handle_positions: [Vec2; 2],
}

impl ProfilePoint {
    #[must_use]
    pub fn new(position: Vec2, handle_type: HandleType) -> ProfilePoint {
        ProfilePoint {
            position,
            handle_types: [handle_type; 2],
            handle_positions: [position; 2],
        }
    }

    #[must_use]
    pub fn with_handles(mut self, before: Vec2, after: Vec2) -> ProfilePoint {
        self.handle_positions = [before, after];
        self
    }

    #[must_use]
    pub fn with_handle_types(mut self, before: HandleType, after: HandleType) -> ProfilePoint {
        self.handle_types = [before, after];
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProfilePreset {
    #[default]
    Line,
    SupportLoops,
    Cornice,
    CrownMolding,
    Steps,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CurveProfile {
    points: Vec<ProfilePoint>,
    sample_straight_edges: bool,
    sample_even_lengths: bool,
}

impl Default for CurveProfile {
    fn default() -> CurveProfile {
        CurveProfile::from_preset(ProfilePreset::Line, 0)
    }
}

impl CurveProfile {
    pub fn new(points: Vec<ProfilePoint>) -> Result<CurveProfile> {
        if points.len() < 2 {
            return Err(Error::ProfileTooShort(points.len()));
        }
        if points.iter().any(|point| !point.position.is_finite()) {
            return Err(Error::ProfileNotFinite);
        }
        Ok(CurveProfile {
            points,
            sample_straight_edges: false,
            sample_even_lengths: false,
        })
    }

    #[must_use]
    pub fn from_preset(preset: ProfilePreset, segments: usize) -> CurveProfile {
        CurveProfile {
            points: preset_points(preset, segments),
            sample_straight_edges: false,
            sample_even_lengths: false,
        }
    }

    #[must_use]
    pub fn with_sample_straight_edges(mut self, enabled: bool) -> CurveProfile {
        self.sample_straight_edges = enabled;
        self
    }

    #[must_use]
    pub fn with_sample_even_lengths(mut self, enabled: bool) -> CurveProfile {
        self.sample_even_lengths = enabled;
        self
    }

    #[must_use]
    pub fn points(&self) -> &[ProfilePoint] {
        &self.points
    }

    #[must_use]
    pub fn samples_straight_edges(&self) -> bool {
        self.sample_straight_edges
    }

    #[must_use]
    pub fn samples_even_lengths(&self) -> bool {
        self.sample_even_lengths
    }

    #[must_use]
    pub fn sample(&self, segments: usize) -> Vec<Vec2> {
        if segments == 0 {
            return Vec::new();
        }
        match self.sample_even_lengths {
            true => even_length_samples(&self.points, segments),
            false => path_samples(&self.points, segments, self.sample_straight_edges),
        }
    }
}

fn point(x: f32, y: f32, handle_type: HandleType) -> ProfilePoint {
    ProfilePoint::new(Vec2::new(x, y), handle_type)
}

fn preset_points(preset: ProfilePreset, segments: usize) -> Vec<ProfilePoint> {
    use HandleType::{Auto, Vector};
    match preset {
        ProfilePreset::Line => vec![point(1.0, 0.0, Auto), point(0.0, 1.0, Auto)],
        ProfilePreset::SupportLoops => support_points(match segments < 4 {
            true => 5,
            false => segments + 1,
        }),
        ProfilePreset::Cornice => vec![
            point(1.0, 0.0, Vector),
            point(1.0, 0.125, Vector),
            point(0.92, 0.16, Auto),
            point(0.875, 0.25, Vector),
            point(0.8, 0.25, Vector),
            point(0.733, 0.433, Auto),
            point(0.582, 0.522, Auto),
            point(0.4, 0.6, Auto),
            point(0.289, 0.727, Auto),
            point(0.25, 0.925, Vector),
            point(0.175, 0.925, Vector),
            point(0.175, 1.0, Vector),
            point(0.0, 1.0, Vector),
        ],
        ProfilePreset::CrownMolding => vec![
            point(1.0, 0.0, Vector),
            point(1.0, 0.25, Vector),
            point(0.75, 0.25, Vector),
            point(0.75, 0.325, Vector),
            point(0.925, 0.4, Auto),
            point(0.975, 0.5, Auto),
            point(0.94, 0.65, Auto),
            point(0.85, 0.75, Auto),
            point(0.75, 0.875, Auto),
            point(0.7, 1.0, Vector),
            point(0.0, 1.0, Vector),
        ],
        ProfilePreset::Steps => step_points(match segments {
            0 => 17,
            count => count + 1,
        }),
    }
}

fn support_points(count: usize) -> Vec<ProfilePoint> {
    use HandleType::{Auto, Vector};
    let mut points = vec![point(1.0, 0.0, Vector), point(1.0, 0.5, Vector)];
    points.resize(count - 2, point(0.0, 0.0, Auto));
    for (i, slot) in points.iter_mut().enumerate().take(count - 2).skip(1) {
        let t = i as f32 / (count - 3) as f32;
        let angle = (f64::from(t) * std::f64::consts::FRAC_PI_2) as f32;
        let x = 1.0 - (0.5 * (1.0 - angle.cos()));
        let y = 0.5 + 0.5 * angle.sin();
        *slot = point(x, y, Auto);
    }
    points.push(point(0.5, 1.0, Vector));
    points.push(point(0.0, 1.0, Vector));
    points
}

fn step_points(count: usize) -> Vec<ProfilePoint> {
    use HandleType::Vector;
    if count == 2 {
        return vec![point(0.0, 1.0, Vector), ProfilePoint::default_free()];
    }
    let steps_x = match count.is_multiple_of(2) {
        true => count,
        false => count - 1,
    } as f32;
    let steps_y = match count.is_multiple_of(2) {
        true => count - 2,
        false => count - 1,
    } as f32;
    (0..count)
        .map(|i| {
            let step_x = i.div_ceil(2);
            let step_y = i / 2;
            point(
                1.0 - (2 * step_x) as f32 / steps_x,
                (2 * step_y) as f32 / steps_y,
                Vector,
            )
        })
        .collect()
}

impl ProfilePoint {
    fn default_free() -> ProfilePoint {
        ProfilePoint {
            position: Vec2::ZERO,
            handle_types: [HandleType::Free; 2],
            handle_positions: [Vec2::ZERO; 2],
        }
    }
}
