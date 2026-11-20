use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use files_core::Location;
use rikka_mime::MimeType;
use rikka_open::is_runnable;
use rikka_xdg::{Exec, Expansion, KeyFile};

use crate::bundled::Bundled;
use crate::run;

#[derive(Clone, Debug)]
struct Thumbnailer {
    exec: String,
    types: Vec<MimeType>,
}

#[derive(Debug, Default)]
pub struct Thumbnailers {
    bundled: Option<Bundled>,
    by_type: HashMap<MimeType, Thumbnailer>,
}

impl Thumbnailers {
    #[must_use]
    pub fn load() -> Self {
        let mut catalog = Self {
            bundled: Bundled::locate(),
            by_type: HashMap::new(),
        };
        for dir in rikka_dirs::data_dirs() {
            let Ok(reading) = std::fs::read_dir(dir.join("thumbnailers")) else {
                continue;
            };
            for entry in reading.flatten() {
                if entry
                    .path()
                    .extension()
                    .is_none_or(|kind| kind != "thumbnailer")
                {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(entry.path()) else {
                    continue;
                };
                if let Some(thumbnailer) = parse(&text) {
                    for mime in &thumbnailer.types {
                        catalog
                            .by_type
                            .entry(mime.clone())
                            .or_insert_with(|| thumbnailer.clone());
                    }
                }
            }
        }
        catalog
    }

    #[must_use]
    pub fn handles(&self, mime: &MimeType) -> bool {
        self.bundled_for(mime).is_some() || self.by_type.contains_key(mime)
    }

    #[must_use]
    pub fn generate(
        &self,
        mime: &MimeType,
        input: &Path,
        at: &Location,
        output: &Path,
        size: u32,
    ) -> bool {
        let command = match self.bundled_for(mime) {
            Some(bundled) => Some(bundled.command(input, output, size)),
            None => self
                .by_type
                .get(mime)
                .and_then(|thumbnailer| thumbnailer.command(input, at, output, size)),
        };
        command.is_some_and(|mut command| run::succeeded(&mut command, run::PATIENCE))
    }

    fn bundled_for(&self, mime: &MimeType) -> Option<&Bundled> {
        self.bundled.as_ref().filter(|_| Bundled::handles(mime))
    }
}

impl Thumbnailer {
    fn command(&self, input: &Path, at: &Location, output: &Path, size: u32) -> Option<Command> {
        let mut parts = expand(&self.exec, input, at, output, size)?;
        let program = parts.remove(0);
        let mut command = Command::new(program);
        command.args(parts);
        Some(command)
    }
}

fn parse(text: &str) -> Option<Thumbnailer> {
    let file = KeyFile::parse(text);
    let fields = file.group("Thumbnailer Entry")?;

    if let Some(try_exec) = fields.string("TryExec")
        && !is_runnable(&try_exec)
    {
        return None;
    }
    let types: Vec<MimeType> = fields
        .list("MimeType", &[])
        .iter()
        .filter_map(|one| MimeType::parse(one.trim()))
        .collect();
    let exec = fields.string("Exec")?;
    (!types.is_empty()).then_some(Thumbnailer { exec, types })
}

fn expand(
    exec: &str,
    input: &Path,
    uri: &Location,
    output: &Path,
    size: u32,
) -> Option<Vec<String>> {
    let parts = Exec::expand(exec, &|code| match code {
        'i' => Expansion::Word(input.to_string_lossy().into_owned()),
        'u' => Expansion::Word(uri.as_str().to_owned()),
        'o' => Expansion::Word(output.to_string_lossy().into_owned()),
        's' => Expansion::Word(size.to_string()),
        _ => Expansion::Nothing,
    });
    (!parts.is_empty()).then_some(parts)
}
