use std::collections::BTreeMap;

use crate::config::drop_in::{self, DropIn};
use crate::error::Result;
use crate::format::SampleFormat;

const MONITOR: &str = "wireplumber/wireplumber.conf.d";

pub(crate) const FORMAT: &str = "audio.format";
pub(crate) const RATE: &str = "audio.rate";
pub(crate) const ALLOWED_RATES: &str = "audio.allowed-rates";
const PERIOD: &str = "api.alsa.period-size";
const HEADROOM: &str = "api.alsa.headroom";
const BATCH: &str = "api.alsa.disable-batch";

const IN: &str = "        ";

pub const PERIODS: [u32; 8] = [32, 64, 128, 256, 512, 1024, 2048, 4096];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceTuning {
    pub format: Option<SampleFormat>,
    pub rate: Option<u32>,
    pub allowed_rates: Vec<u32>,
    pub period: Option<u32>,
    pub headroom: Option<u32>,
    pub disable_batch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Setting {
    Text(String),
    Number(u32),
    Switch(bool),
}

impl DeviceTuning {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn settings(&self) -> [(&'static str, Setting); 6] {
        [
            (
                FORMAT,
                Setting::Text(self.format.map_or("", SampleFormat::name).to_owned()),
            ),
            (RATE, Setting::Number(self.rate.unwrap_or(0))),
            (
                ALLOWED_RATES,
                Setting::Text(drop_in::format_list(&self.allowed_rates)),
            ),
            (PERIOD, Setting::Number(self.period.unwrap_or(0))),
            (HEADROOM, Setting::Number(self.headroom.unwrap_or(0))),
            (BATCH, Setting::Switch(self.disable_batch)),
        ]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceTunings(BTreeMap<String, DeviceTuning>);

impl DeviceTunings {
    pub fn read() -> Self {
        let body = DropIn::in_directory(MONITOR)
            .map(|file| file.read())
            .unwrap_or_default();
        Self(parse(&body))
    }

    pub fn get(&self, node: &str) -> Option<&DeviceTuning> {
        self.0.get(node)
    }

    pub fn set(&mut self, node: &str, tuning: DeviceTuning) {
        if tuning.is_empty() {
            self.0.remove(node);
        } else {
            self.0.insert(node.to_owned(), tuning);
        }
    }

    pub fn write(&self) -> Result<()> {
        let Some(file) = DropIn::in_directory(MONITOR) else {
            return Ok(());
        };
        file.write(&self.body())
    }

    fn body(&self) -> String {
        if self.0.is_empty() {
            return String::new();
        }
        let rules: Vec<String> = self
            .0
            .iter()
            .map(|(node, tuning)| rule(node, tuning))
            .collect();
        format!("monitor.alsa.rules = [\n{}]\n", rules.concat())
    }
}

fn rule(node: &str, tuning: &DeviceTuning) -> String {
    let props: Vec<String> = [
        drop_in::line(
            IN,
            FORMAT,
            tuning.format.map(|f| format!("\"{}\"", f.name())),
        ),
        drop_in::line(IN, RATE, tuning.rate),
        drop_in::line(IN, ALLOWED_RATES, drop_in::list(&tuning.allowed_rates)),
        drop_in::line(IN, PERIOD, tuning.period),
        drop_in::line(IN, HEADROOM, tuning.headroom),
        drop_in::line(IN, BATCH, tuning.disable_batch.then_some(true)),
    ]
    .into_iter()
    .flatten()
    .collect();
    format!(
        "  {{\n    matches = [ {{ node.name = \"{node}\" }} ]\n    \
         actions = {{ update-props = {{\n{}\n    }} }}\n  }}\n",
        props.join("\n")
    )
}

fn parse(body: &str) -> BTreeMap<String, DeviceTuning> {
    let mut rules = BTreeMap::new();
    for block in body.split("node.name = ").skip(1) {
        let Some(node) = block
            .strip_prefix('"')
            .and_then(|rest| rest.split('"').next())
        else {
            continue;
        };
        rules.insert(
            node.to_owned(),
            DeviceTuning {
                format: drop_in::text(block, FORMAT).and_then(SampleFormat::from_name),
                rate: drop_in::number(block, RATE),
                allowed_rates: drop_in::numbers(block, ALLOWED_RATES),
                period: drop_in::number(block, PERIOD),
                headroom: drop_in::number(block, HEADROOM),
                disable_batch: drop_in::setting(block, BATCH) == Some("true"),
            },
        );
    }
    rules
}
