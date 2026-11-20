use glam::Vec3;

const PLANET_RADIUS: f32 = 6360.0;
const ATMOSPHERE_RADIUS: f32 = 6460.0;
const RAYLEIGH_SCATTERING: Vec3 = Vec3::new(5.802e-3, 13.558e-3, 33.1e-3);
const RAYLEIGH_HEIGHT: f32 = 8.0;
const MIE_EXTINCTION: f32 = 4.44e-3;
const MIE_HEIGHT: f32 = 1.2;
const OZONE_ABSORPTION: Vec3 = Vec3::new(0.650e-3, 1.881e-3, 0.085e-3);
const OZONE_CENTER: f32 = 25.0;
const OZONE_WIDTH: f32 = 15.0;
const VIEWER_ALTITUDE: f32 = 0.2;

const STEPS: u32 = 40;

fn extinction_at(altitude: f32, haze: f32) -> Vec3 {
    let rayleigh = RAYLEIGH_SCATTERING * (-altitude / RAYLEIGH_HEIGHT).exp();
    let mie = MIE_EXTINCTION * haze * (-altitude / MIE_HEIGHT).exp();
    let ozone = OZONE_ABSORPTION * (1.0 - (altitude - OZONE_CENTER).abs() / OZONE_WIDTH).max(0.0);
    rayleigh + Vec3::splat(mie) + ozone
}

pub(crate) fn transmittance(towards: Vec3, haze: f32) -> Vec3 {
    let origin = Vec3::new(0.0, PLANET_RADIUS + VIEWER_ALTITUDE, 0.0);
    let direction = towards.normalize_or(Vec3::Y);
    if hits_sphere(origin, direction, PLANET_RADIUS) {
        return Vec3::ZERO;
    }
    let Some(length) = exit_distance(origin, direction, ATMOSPHERE_RADIUS) else {
        return Vec3::ONE;
    };
    let step = length / STEPS as f32;
    let mut depth = Vec3::ZERO;
    for index in 0..STEPS {
        let point = origin + direction * ((index as f32 + 0.5) * step);
        depth += extinction_at(point.length() - PLANET_RADIUS, haze) * step;
    }
    Vec3::new((-depth.x).exp(), (-depth.y).exp(), (-depth.z).exp())
}

fn exit_distance(origin: Vec3, direction: Vec3, radius: f32) -> Option<f32> {
    let b = origin.dot(direction);
    let c = origin.length_squared() - radius * radius;
    let discriminant = b * b - c;
    (discriminant >= 0.0).then(|| -b + discriminant.sqrt())
}

fn hits_sphere(origin: Vec3, direction: Vec3, radius: f32) -> bool {
    let b = origin.dot(direction);
    let c = origin.length_squared() - radius * radius;
    b < 0.0 && b * b - c >= 0.0
}
