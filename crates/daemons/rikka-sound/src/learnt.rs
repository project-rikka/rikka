use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::format::SampleFormat;
use crate::hardware::Hardware;

const FILE: &str = "sound/hardware";

#[derive(Debug, Default)]
pub(crate) struct Learnt {
    path: Option<PathBuf>,
    known: BTreeMap<String, Hardware>,
}

impl Learnt {
    pub(crate) fn load() -> Learnt {
        let path = rikka_dirs::Paths::resolve()
            .ok()
            .map(|paths| paths.state_dir().join(FILE));
        let known = path.as_deref().map(read).unwrap_or_default();
        Learnt { path, known }
    }

    pub(crate) fn get(&self, node: &str) -> Hardware {
        self.known.get(node).cloned().unwrap_or_default()
    }

    pub(crate) fn learn(&mut self, node: &str, observed: &Hardware) -> Hardware {
        let known = self.get(node);
        let widened = known.widened(observed);
        if widened != known && recordable(node) {
            self.known.insert(node.to_owned(), widened.clone());
            self.save();
        }
        widened
    }

    fn save(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        for (node, elsewhere) in read(path) {
            let merged = self.get(&node).widened(&elsewhere);
            self.known.insert(node, merged);
        }
        if let Err(error) = rikka_fs::replace(path, serialize_known(&self.known)) {
            tracing::debug!(
                "failed to save learned card rates to {}: {error}",
                path.display()
            );
        }
    }
}

fn recordable(node: &str) -> bool {
    !node.is_empty() && !node.contains(['\t', '\n'])
}

fn read(path: &Path) -> BTreeMap<String, Hardware> {
    std::fs::read_to_string(path)
        .map(|body| parse(&body))
        .unwrap_or_default()
}

fn parse(body: &str) -> BTreeMap<String, Hardware> {
    body.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let node = fields.next().filter(|node| !node.is_empty())?;
            let formats = fields
                .next()?
                .split_whitespace()
                .filter_map(SampleFormat::from_name)
                .collect();
            let rates = fields.next()?.split_once('-').and_then(|(low, high)| {
                let (low, high) = (low.parse().ok()?, high.parse().ok()?);
                (low <= high).then_some(low..=high)
            });
            let hardware = Hardware {
                formats,
                rates,
                refuses_pinned_format: false,
            };
            Some((node.to_owned(), Hardware::default().widened(&hardware)))
        })
        .collect()
}

fn serialize_known(known: &BTreeMap<String, Hardware>) -> String {
    known
        .iter()
        .map(|(node, hardware)| {
            let formats: Vec<&str> = hardware.formats.iter().map(|f| f.name()).collect();
            let rates = hardware
                .rates
                .as_ref()
                .map(|rates| format!("{}-{}", rates.start(), rates.end()))
                .unwrap_or_default();
            format!("{node}\t{}\t{rates}\n", formats.join(" "))
        })
        .collect()
}
