use std::time::{Duration, SystemTime};

use zbus::blocking::Connection;

use crate::proxies::DeviceProxy;
use crate::reading::Charge;

const CHARGE: &str = "charge";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub at: SystemTime,
    pub charge: f32,
    pub state: Charge,
}

pub(crate) fn read(
    connection: &Connection,
    pack: &str,
    span: Duration,
    points: u32,
) -> zbus::Result<Vec<Sample>> {
    let device = DeviceProxy::builder(connection)
        .path(pack.to_owned())?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()?;
    let seconds = u32::try_from(span.as_secs()).unwrap_or(u32::MAX);
    let mut samples: Vec<Sample> = device
        .get_history(CHARGE, seconds, points)?
        .into_iter()
        .filter(|(at, _, _)| *at > 0)
        .map(|(at, value, state)| Sample {
            at: SystemTime::UNIX_EPOCH + Duration::from_secs(u64::from(at)),
            charge: (value / 100.0).clamp(0.0, 1.0) as f32,
            state: Charge::from_code(state),
        })
        .collect();
    samples.sort_by_key(|sample| sample.at);
    Ok(samples)
}
