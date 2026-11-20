use crate::config::drop_in::{self, DropIn};
use crate::error::Result;

const DAEMON: &str = "pipewire/pipewire.conf.d";

const CLIENTS: [&str; 2] = ["pipewire/client.conf.d", "pipewire/client-rt.conf.d"];

const RATE: &str = "default.clock.rate";
const ALLOWED_RATES: &str = "default.clock.allowed-rates";
const QUANTUM: &str = "default.clock.quantum";
const MIN_QUANTUM: &str = "default.clock.min-quantum";
const MAX_QUANTUM: &str = "default.clock.max-quantum";
const RESAMPLE_QUALITY: &str = "resample.quality";

const IN: &str = "    ";

pub const RESAMPLE_QUALITIES: std::ops::RangeInclusive<u32> = 0..=14;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphTuning {
    pub rate: Option<u32>,
    pub allowed_rates: Vec<u32>,
    pub quantum: Option<u32>,
    pub min_quantum: Option<u32>,
    pub max_quantum: Option<u32>,
    pub resample_quality: Option<u32>,
}

impl GraphTuning {
    pub fn read() -> Self {
        let daemon = DropIn::in_directory(DAEMON)
            .map(|file| file.read())
            .unwrap_or_default();
        let client = DropIn::in_directory(CLIENTS[0])
            .map(|file| file.read())
            .unwrap_or_default();
        Self {
            rate: drop_in::number(&daemon, RATE),
            allowed_rates: drop_in::numbers(&daemon, ALLOWED_RATES),
            quantum: drop_in::number(&daemon, QUANTUM),
            min_quantum: drop_in::number(&daemon, MIN_QUANTUM),
            max_quantum: drop_in::number(&daemon, MAX_QUANTUM),
            resample_quality: drop_in::number(&client, RESAMPLE_QUALITY),
        }
    }

    pub fn write(&self) -> Result<()> {
        if let Some(file) = DropIn::in_directory(DAEMON) {
            file.write(&self.daemon_body())?;
        }
        let stream = self.client_body();
        for directory in CLIENTS {
            if let Some(file) = DropIn::in_directory(directory) {
                file.write(&stream)?;
            }
        }
        Ok(())
    }

    fn daemon_body(&self) -> String {
        block(
            "context.properties",
            [
                drop_in::line(IN, RATE, self.rate),
                drop_in::line(IN, ALLOWED_RATES, drop_in::list(&self.allowed_rates)),
                drop_in::line(IN, QUANTUM, self.quantum),
                drop_in::line(IN, MIN_QUANTUM, self.min_quantum),
                drop_in::line(IN, MAX_QUANTUM, self.max_quantum),
            ],
        )
    }

    fn client_body(&self) -> String {
        block(
            "stream.properties",
            [drop_in::line(IN, RESAMPLE_QUALITY, self.resample_quality)],
        )
    }
}

fn block(name: &str, settings: impl IntoIterator<Item = Option<String>>) -> String {
    let settings: Vec<String> = settings.into_iter().flatten().collect();
    if settings.is_empty() {
        return String::new();
    }
    format!("{name} = {{\n{}\n}}\n", settings.join("\n"))
}
