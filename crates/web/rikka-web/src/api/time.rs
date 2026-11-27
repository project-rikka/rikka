use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn from_millis(millis: f64) -> SystemTime {
    if millis.is_finite() && millis > 0.0 {
        UNIX_EPOCH + Duration::from_secs_f64(millis / 1000.0)
    } else {
        UNIX_EPOCH
    }
}

pub(crate) fn to_millis(time: SystemTime) -> f64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64() * 1000.0)
}

pub(crate) mod millis {
    pub(crate) mod optional {
        use std::time::SystemTime;

        use serde::{Deserialize, Deserializer};

        pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<SystemTime>, D::Error> {
            Ok(Option::<f64>::deserialize(deserializer)?.map(super::super::from_millis))
        }
    }
}

pub(crate) mod iso {
    use std::time::SystemTime;

    use serde::{Deserialize, Deserializer};

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<SystemTime>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        text.map(|text| {
            text.parse::<jiff::Timestamp>()
                .map(SystemTime::from)
                .map_err(serde::de::Error::custom)
        })
        .transpose()
    }
}
