use animate_wind::{WindKey, WindLevels, WindMotion, WindSetting};
use rikka_mmd::VmdCurve;
use rikka_model_core::glam::Vec3;

use crate::support::key;

fn heading(yaw: f32, pitch: f32) -> Vec3 {
    WindLevels {
        yaw,
        pitch,
        ..WindLevels::default()
    }
    .direction()
}

#[test]
fn heading_turns_counterclockwise_from_front() {
    assert!(heading(0.0, 0.0).abs_diff_eq(Vec3::NEG_Z, 1e-6));
    assert!(heading(90.0, 0.0).abs_diff_eq(Vec3::NEG_X, 1e-6));
    assert!(heading(180.0, 0.0).abs_diff_eq(Vec3::Z, 1e-6));
    assert!(heading(-90.0, 0.0).abs_diff_eq(Vec3::X, 1e-6));
    assert!(heading(0.0, 90.0).abs_diff_eq(Vec3::Y, 1e-6));
    assert!(
        heading(45.0, -30.0).y < 0.0,
        "a negative pitch blows downward"
    );
}

#[test]
fn unkeyed_wind_uses_rest_levels() {
    let mut motion = WindMotion::default();
    assert!(motion.is_empty());
    assert_eq!(motion.sample(40.0), WindLevels::default().wind());
    motion.levels.speed = 3.0;
    assert!(!motion.is_empty());
    assert_eq!(motion.sample(40.0).speed, 3.0);

    motion.tracks_mut().insert(key(WindSetting::Speed, 10, 2.0));
    motion.tracks_mut().insert(WindKey {
        curve: VmdCurve {
            first: (127, 0),
            second: (127, 0),
        },
        ..key(WindSetting::Speed, 20, 4.0)
    });
    motion.tracks_mut().insert(key(WindSetting::Yaw, 0, 0.0));
    motion.tracks_mut().insert(key(WindSetting::Yaw, 30, 90.0));
    assert_eq!(
        motion.sample(0.0).speed,
        2.0,
        "before the first key it holds"
    );
    let eased = motion.sample(15.0).speed;
    assert!(eased > 2.0 && eased < 3.0, "{eased}");
    assert_eq!(motion.sample(99.0).speed, 4.0);
    assert_eq!(
        motion.levels_at(99.0).turbulence,
        WindLevels::default().turbulence
    );
    let quarter = motion.sample(15.0).direction;
    let expected = Vec3::new(
        -(45f32.to_radians().sin()),
        0.0,
        -(45f32.to_radians().cos()),
    );
    assert!(quarter.abs_diff_eq(expected, 1e-5), "{quarter}");
    assert_eq!(motion.last_frame(), 30);
}

#[test]
fn heading_interpolates_across_back() {
    let mut motion = WindMotion::default();
    motion.tracks_mut().insert(key(WindSetting::Yaw, 0, 160.0));
    motion
        .tracks_mut()
        .insert(key(WindSetting::Yaw, 10, -160.0));
    let halfway = motion.value_at(WindSetting::Yaw, 5.0);
    assert_eq!(halfway.abs(), 180.0);
    assert!(motion.sample(5.0).direction.abs_diff_eq(Vec3::Z, 1e-5));
}
