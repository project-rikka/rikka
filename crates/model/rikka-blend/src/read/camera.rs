use rikka_model_core::{Camera, Projection};

use super::modifier::Notes;
use crate::file::Record;

const ORTHOGRAPHIC: i64 = 1;
const PANORAMIC: i64 = 2;
const FIT_HORIZONTAL: i64 = 1;
const FIT_VERTICAL: i64 = 2;

pub(crate) fn aspect(scene: Option<Record<'_>>) -> f32 {
    let Some(render) = scene.and_then(|scene| scene.record("r")) else {
        return 16.0 / 9.0;
    };
    let width = render.int("xsch").unwrap_or(1920) as f32 * render.float("xasp").unwrap_or(1.0);
    let height = render.int("ysch").unwrap_or(1080) as f32 * render.float("yasp").unwrap_or(1.0);
    match width / height {
        aspect if aspect.is_finite() && aspect > 0.0 => aspect,
        _ => 16.0 / 9.0,
    }
}

pub(crate) fn camera(record: &Record<'_>, aspect: f32, notes: &mut Notes) -> Camera {
    let name = record.id_name();
    let kind = record.int("type").unwrap_or(0);
    let fit = record.int("sensor_fit").unwrap_or(0);
    let horizontal = match fit {
        FIT_HORIZONTAL => true,
        FIT_VERTICAL => false,
        _ => aspect >= 1.0,
    };
    let near = record
        .float("clipsta")
        .or_else(|| record.float("clip_start"))
        .unwrap_or(0.1);
    let far = record
        .float("clipend")
        .or_else(|| record.float("clip_end"))
        .unwrap_or(100.0);
    if kind == PANORAMIC {
        notes.unsupported(format!("panoramic camera \"{name}\" read as perspective"));
    }
    let projection = match kind {
        ORTHOGRAPHIC => {
            let scale = record.float("ortho_scale").unwrap_or(6.0);
            let (width, height) = match horizontal {
                true => (scale, scale / aspect),
                false => (scale * aspect, scale),
            };
            Projection::Orthographic {
                width,
                height,
                near,
                far,
            }
        }
        _ => {
            let lens = record.float("lens").unwrap_or(50.0).max(1.0e-3);
            let sensor = match fit {
                FIT_VERTICAL => record.float("sensor_y").unwrap_or(24.0),
                _ => record.float("sensor_x").unwrap_or(36.0),
            };
            let half = (sensor / (2.0 * lens)).atan();
            let y_fov = match horizontal {
                true => 2.0 * (half.tan() / aspect).atan(),
                false => 2.0 * half,
            };
            Projection::Perspective {
                y_fov,
                aspect: Some(aspect),
                near,
                far: Some(far),
            }
        }
    };
    Camera::new(name).projection(projection)
}
