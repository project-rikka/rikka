use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::reading;

const HWMON: &str = "/sys/class/hwmon";
const REDISCOVERED: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sensors {
    pub thermals: Vec<Thermal>,
    pub fans: Vec<Fan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Thermal {
    pub chip: String,
    pub label: String,
    pub celsius: f32,
    pub high: Option<f32>,
    pub critical: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fan {
    pub chip: String,
    pub label: String,
    pub rpm: u32,
}

impl Sensors {
    #[must_use]
    pub fn cpu(&self) -> Option<f32> {
        let any = |_: &str| true;
        ["k10temp", "zenpower"]
            .into_iter()
            .find_map(|chip| {
                self.hottest_on(chip, |label| label == "Tdie")
                    .or_else(|| self.hottest_on(chip, |label| label == "Tctl"))
            })
            .or_else(|| self.hottest_on("coretemp", |label| label.starts_with("Package id")))
            .or_else(|| self.hottest_on("coretemp", any))
            .or_else(|| {
                ["cpu_thermal", "cpu-thermal", "soc_thermal"]
                    .into_iter()
                    .find_map(|chip| self.hottest_on(chip, any))
            })
            .or_else(|| {
                self.thermals
                    .iter()
                    .find(|thermal| thermal.label == "CPU")
                    .map(|thermal| thermal.celsius)
            })
            .or_else(|| self.hottest_on("acpitz", any))
    }

    fn hottest_on(&self, chip: &str, labelled: impl Fn(&str) -> bool) -> Option<f32> {
        self.thermals
            .iter()
            .filter(|thermal| thermal.chip == chip && labelled(&thermal.label))
            .map(|thermal| thermal.celsius)
            .reduce(f32::max)
    }

    #[must_use]
    pub fn hottest(&self) -> Option<&Thermal> {
        self.thermals
            .iter()
            .max_by(|left, right| left.celsius.total_cmp(&right.celsius))
    }
}

#[derive(Debug)]
struct Input {
    chip: String,
    label: String,
    value: PathBuf,
    high: Option<PathBuf>,
    critical: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Thermal,
    Fan,
}

#[derive(Debug)]
pub(crate) struct Hwmon {
    root: PathBuf,
    thermals: Vec<Input>,
    fans: Vec<Input>,
    found_at: Instant,
}

impl Hwmon {
    pub(crate) fn new() -> Hwmon {
        Hwmon::with_root(HWMON)
    }

    fn with_root(root: impl Into<PathBuf>) -> Hwmon {
        let root = root.into();
        let (thermals, fans) = discover(&root);
        Hwmon {
            root,
            thermals,
            fans,
            found_at: Instant::now(),
        }
    }

    pub(crate) fn read(&mut self) -> Sensors {
        if self.found_at.elapsed() >= REDISCOVERED {
            (self.thermals, self.fans) = discover(&self.root);
            self.found_at = Instant::now();
        }
        let degrees = |path: &Path| reading::number::<f32>(path).map(|milli| milli / 1000.0);
        Sensors {
            thermals: self
                .thermals
                .iter()
                .filter_map(|input| {
                    Some(Thermal {
                        chip: input.chip.clone(),
                        label: input.label.clone(),
                        celsius: degrees(&input.value).filter(|celsius| plausible(*celsius))?,
                        high: input.high.as_deref().and_then(degrees).filter(|c| *c > 0.0),
                        critical: input
                            .critical
                            .as_deref()
                            .and_then(degrees)
                            .filter(|c| *c > 0.0),
                    })
                })
                .collect(),
            fans: self
                .fans
                .iter()
                .filter_map(|input| {
                    Some(Fan {
                        chip: input.chip.clone(),
                        label: input.label.clone(),
                        rpm: reading::number(&input.value)?,
                    })
                })
                .collect(),
        }
    }
}

fn plausible(celsius: f32) -> bool {
    (-40.0..=200.0).contains(&celsius) && celsius != 0.0
}

fn discover(root: &Path) -> (Vec<Input>, Vec<Input>) {
    let mut chips = reading::entries(root);
    chips.sort_by_key(|chip| natural(chip));
    let mut thermals = Vec::new();
    let mut fans = Vec::new();
    for chip in chips {
        let dir = root.join(&chip);
        let name = reading::text(dir.join("name"))
            .or_else(|| reading::text(dir.join("device/name")))
            .unwrap_or(chip);
        let mut inputs: Vec<(Kind, u32, String)> = reading::entries(&dir)
            .into_iter()
            .filter_map(|file| input(&file).map(|(kind, index)| (kind, index, file)))
            .collect();
        inputs.sort_by_key(|(_, index, _)| *index);
        for (kind, index, file) in inputs {
            let prefix = match kind {
                Kind::Thermal => format!("temp{index}"),
                Kind::Fan => format!("fan{index}"),
            };
            let found = Input {
                chip: name.clone(),
                label: reading::text(dir.join(format!("{prefix}_label")))
                    .filter(|label| !label.is_empty())
                    .unwrap_or_else(|| prefix.clone()),
                value: dir.join(&file),
                high: Some(dir.join(format!("{prefix}_max"))).filter(|path| path.exists()),
                critical: Some(dir.join(format!("{prefix}_crit"))).filter(|path| path.exists()),
            };
            match kind {
                Kind::Thermal => thermals.push(found),
                Kind::Fan => fans.push(found),
            }
        }
    }
    (thermals, fans)
}

fn input(file: &str) -> Option<(Kind, u32)> {
    let stem = file.strip_suffix("_input")?;
    let (kind, index) = match stem.strip_prefix("temp") {
        Some(index) => (Kind::Thermal, index),
        None => (Kind::Fan, stem.strip_prefix("fan")?),
    };
    Some((kind, index.parse().ok()?))
}

fn natural(name: &str) -> (String, u64) {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (stem, number) = name.split_at(name.len() - digits);
    (stem.to_owned(), number.parse().unwrap_or_default())
}
