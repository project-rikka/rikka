use glam::{Mat3, Quat, Vec3};

use crate::light::ShadowQuality;

const AXIAL_TILT: f32 = 23.44;
const DAYS_PER_YEAR: f32 = 365.0;
const SOLSTICE_OFFSET: f32 = 10.0;
const EQUINOX_DAY: f32 = 80.0;
const HOURS_PER_DAY: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub hour: f32,
    pub heading: f32,
    pub latitude: f32,
    pub day: f32,
    pub haze: f32,
    pub sun_size: f32,
    pub sun: f32,
    pub moon: f32,
    pub moon_phase: f32,
    pub stars: f32,
    pub clouds: f32,
    pub cloud_drift: f32,
    pub ground: Vec3,
    pub shadows: ShadowQuality,
}

impl Default for Sky {
    fn default() -> Self {
        Self {
            hour: 15.5,
            heading: 0.0,
            latitude: 35.7,
            day: 110.0,
            haze: 1.0,
            sun_size: 0.6,
            sun: 1.0,
            moon: 1.0,
            moon_phase: 0.5,
            stars: 1.0,
            clouds: 0.35,
            cloud_drift: 1.0,
            ground: Vec3::new(0.3, 0.28, 0.25),
            shadows: ShadowQuality::High,
        }
    }
}

impl Sky {
    pub fn to_sun(&self) -> Vec3 {
        let declination = -AXIAL_TILT.to_radians()
            * (std::f32::consts::TAU * (self.day + SOLSTICE_OFFSET) / DAYS_PER_YEAR).cos();
        self.celestial(self.hour_angle(), declination)
    }

    pub fn to_moon(&self) -> Vec3 {
        let solar = -AXIAL_TILT.to_radians()
            * (std::f32::consts::TAU * (self.day + SOLSTICE_OFFSET) / DAYS_PER_YEAR).cos();
        let phase = std::f32::consts::TAU * self.moon_phase;
        self.celestial(self.hour_angle() - phase, solar * phase.cos())
    }

    pub fn moon_illumination(&self) -> f32 {
        0.5 - 0.5 * (std::f32::consts::TAU * self.moon_phase).cos()
    }

    pub(crate) fn star_rotation(&self) -> Mat3 {
        let (sin_l, cos_l) = self.latitude.clamp(-89.9, 89.9).to_radians().sin_cos();
        let equatorial = Mat3::from_cols(
            Vec3::new(0.0, cos_l, sin_l),
            Vec3::X,
            Vec3::new(0.0, sin_l, -cos_l),
        )
        .transpose();
        let solar_ascension = std::f32::consts::TAU * (self.day - EQUINOX_DAY) / DAYS_PER_YEAR;
        let sidereal = self.hour_angle() + solar_ascension;
        Mat3::from_rotation_z(sidereal) * equatorial * Mat3::from_rotation_y(-self.heading)
    }

    fn hour_angle(&self) -> f32 {
        let hour = self.hour.rem_euclid(HOURS_PER_DAY);
        (hour - HOURS_PER_DAY * 0.5) * std::f32::consts::TAU / HOURS_PER_DAY
    }

    fn celestial(&self, hour_angle: f32, declination: f32) -> Vec3 {
        let latitude = self.latitude.clamp(-89.9, 89.9).to_radians();
        let (sin_h, cos_h) = hour_angle.sin_cos();
        let (sin_d, cos_d) = declination.sin_cos();
        let (sin_l, cos_l) = latitude.sin_cos();
        let east = -cos_d * sin_h;
        let north = cos_l * sin_d - sin_l * cos_d * cos_h;
        let up = sin_l * sin_d + cos_l * cos_d * cos_h;
        let local = Vec3::new(east, up, -north).normalize_or(Vec3::Y);
        Quat::from_rotation_y(self.heading) * local
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub density: f32,
    pub height: f32,
    pub color: Vec3,
    pub forward: f32,
    pub shafts: f32,
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            density: 0.0005,
            height: 25.0,
            color: Vec3::ONE,
            forward: 0.6,
            shafts: 1.0,
        }
    }
}
