use std::time::Duration;

const DEFAULT: Duration = Duration::from_secs(120);
const SHORTEST: Duration = Duration::from_secs(10);
const LONGEST: Duration = Duration::from_secs(600);

pub(crate) fn clamp(requested: Option<Duration>) -> Duration {
    requested.map_or(DEFAULT, |requested| requested.clamp(SHORTEST, LONGEST))
}
