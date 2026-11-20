use rikka_net::SpkiPin;

const API: [&str; 6] = [
    "drtmcR2kFkM8qJClsuWgUzxgBkePfRCkRpqUesyDmeE=",
    "YRGlaY0jyJ4Jw2/4M8FIftwbDIQfh8Sdro96CeEel54=",
    "AfMENBVvOS8MnISprtvyPsjKlPooqh8nMB/pvCrpJpw=",
    "CT56BhOTmj5ZIPgb/xD5mH8rY3BLo/MlhP7oPyJUEDo=",
    "35Dx28/uzN3LeltkCBQ8RHK0tlNSa2kCpCRGNp34Gxc=",
    "qYIukVc63DEITct8sFT7ebIq5qsWmuscaIKeJx+5J5A=",
];

const ALTERNATIVE_API: [&str; 4] = [
    "EU6TS9MO0L/GsDHvVc9D5fChYLNy5JdGYpJw0ccgetM=",
    "iKPIHPnDNqdkvOnTClQ8zQAIKG0XavaPkcEo0LBAABA=",
    "MSlVrBCdL0hKyczvgYVSRNm88RicyY04Q2y5qrBt0xA=",
    "C2UxW0T1Ckl9s+8cXfjXxlEqwAfPM4HiW2y3UdtBeCw=",
];

pub(crate) fn api() -> Vec<SpkiPin> {
    parsed(&API)
}

pub(crate) fn alternative_api() -> Vec<SpkiPin> {
    parsed(&ALTERNATIVE_API)
}

fn parsed(pins: &[&str]) -> Vec<SpkiPin> {
    pins.iter()
        .map(|pin| SpkiPin::from_base64(pin).expect("pins should be SHA-256 digests"))
        .collect()
}
