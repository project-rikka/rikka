use rikka_settings::session::weather::{FAHRENHEIT, TEMPERATURE_UNIT};
use rikka_weather::{
    Distance, DistanceUnit, Precipitation, PrecipitationUnit, Pressure, PressureUnit, Speed,
    SpeedUnit, Temperature, TemperatureUnit, Units,
};

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

#[test]
fn units_convert_and_wind_maps_to_beaufort() {
    assert!(near(
        Temperature(100.0).in_unit(TemperatureUnit::Fahrenheit),
        212.0
    ));
    assert!(near(
        Temperature(-40.0).in_unit(TemperatureUnit::Fahrenheit),
        -40.0
    ));
    assert!(near(
        Speed(10.0).in_unit(SpeedUnit::KilometresPerHour),
        36.0
    ));
    assert!(near(Speed(10.0).in_unit(SpeedUnit::Knots), 19.44));
    assert!(near(
        Precipitation(25.4).in_unit(PrecipitationUnit::Inches),
        1.0
    ));
    assert!(near(
        Pressure(1013.25).in_unit(PressureUnit::InchesOfMercury),
        29.92
    ));
    assert!(near(Distance(1609.344).in_unit(DistanceUnit::Miles), 1.0));

    assert_eq!(Speed(0.2).beaufort(), 0);
    assert_eq!(Speed(4.0).beaufort(), 3);
    assert_eq!(Speed(40.0).beaufort(), 12);
}

#[test]
fn configured_unit_overrides_locale() {
    let units =
        Units::from_settings(|key| (key == TEMPERATURE_UNIT).then(|| FAHRENHEIT.to_owned()));

    assert_eq!(units.temperature, TemperatureUnit::Fahrenheit);
    assert_eq!(
        units.speed,
        Units::for_locale().speed,
        "automatic follows the locale"
    );
}
