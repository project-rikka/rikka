use serde::Deserialize;

use crate::air::{Air, Pollen};
use crate::open_meteo::forecast::instant;

pub(crate) const URL: &str = "https://air-quality-api.open-meteo.com/v1/air-quality";

const CURRENT: &str = "us_aqi,european_aqi,pm10,pm2_5,carbon_monoxide,nitrogen_dioxide,sulphur_dioxide,ozone,uv_index,alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen";

pub(crate) fn query(latitude: f64, longitude: f64) -> Vec<(&'static str, String)> {
    vec![
        ("latitude", format!("{latitude:.2}")),
        ("longitude", format!("{longitude:.2}")),
        ("current", CURRENT.to_owned()),
        ("timezone", "auto".to_owned()),
        ("timeformat", "unixtime".to_owned()),
    ]
}

#[derive(Debug, Deserialize)]
pub(crate) struct Answer {
    current: Option<Current>,
}

#[derive(Debug, Deserialize)]
struct Current {
    time: i64,
    us_aqi: Option<f32>,
    european_aqi: Option<f32>,
    pm10: Option<f32>,
    pm2_5: Option<f32>,
    carbon_monoxide: Option<f32>,
    nitrogen_dioxide: Option<f32>,
    sulphur_dioxide: Option<f32>,
    ozone: Option<f32>,
    uv_index: Option<f32>,
    alder_pollen: Option<f32>,
    birch_pollen: Option<f32>,
    grass_pollen: Option<f32>,
    mugwort_pollen: Option<f32>,
    olive_pollen: Option<f32>,
    ragweed_pollen: Option<f32>,
}

fn index(value: Option<f32>) -> Option<u16> {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value.round().min(f32::from(u16::MAX)) as u16)
}

impl Answer {
    pub(crate) fn into_air(self) -> Option<Air> {
        let current = self.current?;
        let pollen = Pollen {
            alder: current.alder_pollen,
            birch: current.birch_pollen,
            grass: current.grass_pollen,
            mugwort: current.mugwort_pollen,
            olive: current.olive_pollen,
            ragweed: current.ragweed_pollen,
        };
        Some(Air {
            at: instant(current.time),
            us_aqi: index(current.us_aqi),
            european_aqi: index(current.european_aqi),
            pm2_5: current.pm2_5,
            pm10: current.pm10,
            ozone: current.ozone,
            nitrogen_dioxide: current.nitrogen_dioxide,
            sulphur_dioxide: current.sulphur_dioxide,
            carbon_monoxide: current.carbon_monoxide,
            uv_index: current.uv_index,
            pollen: (!pollen.is_empty()).then_some(pollen),
        })
    }
}
