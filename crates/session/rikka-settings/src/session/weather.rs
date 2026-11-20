use crate::key::Key;
use crate::schema::Schema;
use crate::value::Values;

pub const DOMAIN: &str = "rikka.weather";

pub const LOCATION: &str = "location";
pub const PLACE: &str = "place";
pub const TEMPERATURE_UNIT: &str = "temperature-unit";
pub const SPEED_UNIT: &str = "speed-unit";
pub const PRECIPITATION_UNIT: &str = "precipitation-unit";
pub const PRESSURE_UNIT: &str = "pressure-unit";
pub const DISTANCE_UNIT: &str = "distance-unit";

pub const HERE: &str = "here";
pub const CHOSEN: &str = "place";
pub const AUTOMATIC: &str = "auto";

pub const CELSIUS: &str = "celsius";
pub const FAHRENHEIT: &str = "fahrenheit";
pub const KILOMETRES_PER_HOUR: &str = "kmh";
pub const METRES_PER_SECOND: &str = "ms";
pub const MILES_PER_HOUR: &str = "mph";
pub const KNOTS: &str = "knots";
pub const MILLIMETRES: &str = "mm";
pub const INCHES: &str = "inch";
pub const HECTOPASCALS: &str = "hpa";
pub const MILLIMETRES_OF_MERCURY: &str = "mmhg";
pub const INCHES_OF_MERCURY: &str = "inhg";
pub const KILOMETRES: &str = "km";
pub const MILES: &str = "mi";

pub const LOCATIONS: [&str; 2] = [HERE, CHOSEN];
pub const TEMPERATURE_UNITS: [&str; 3] = [AUTOMATIC, CELSIUS, FAHRENHEIT];
pub const SPEED_UNITS: [&str; 5] = [
    AUTOMATIC,
    KILOMETRES_PER_HOUR,
    METRES_PER_SECOND,
    MILES_PER_HOUR,
    KNOTS,
];
pub const PRECIPITATION_UNITS: [&str; 3] = [AUTOMATIC, MILLIMETRES, INCHES];
pub const PRESSURE_UNITS: [&str; 4] = [
    AUTOMATIC,
    HECTOPASCALS,
    MILLIMETRES_OF_MERCURY,
    INCHES_OF_MERCURY,
];
pub const DISTANCE_UNITS: [&str; 3] = [AUTOMATIC, KILOMETRES, MILES];

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("Where the desktop forecasts the weather for, and in which units")
        .key(
            Key::string(LOCATION, HERE)
                .summary("Whether the forecast follows where this machine is, or one chosen place")
                .choices(LOCATIONS),
        )
        .key(Key::table(PLACE, Values::new()).summary(
            "The chosen place: name, region, country, country_code, latitude and longitude",
        ))
        .key(
            Key::string(TEMPERATURE_UNIT, AUTOMATIC)
                .summary("How temperatures are given; automatic follows the measurement locale")
                .choices(TEMPERATURE_UNITS),
        )
        .key(
            Key::string(SPEED_UNIT, AUTOMATIC)
                .summary("How wind speeds are given")
                .choices(SPEED_UNITS),
        )
        .key(
            Key::string(PRECIPITATION_UNIT, AUTOMATIC)
                .summary("How rain and snow are measured")
                .choices(PRECIPITATION_UNITS),
        )
        .key(
            Key::string(PRESSURE_UNIT, AUTOMATIC)
                .summary("How air pressure is given")
                .choices(PRESSURE_UNITS),
        )
        .key(
            Key::string(DISTANCE_UNIT, AUTOMATIC)
                .summary("How far one can see is given")
                .choices(DISTANCE_UNITS),
        )
}
