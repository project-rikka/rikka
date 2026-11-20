use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::host;
use crate::shipped::{APPLICATIONS_DIRECTORY, About, Component};

const GROUP: &str = "[Desktop Entry]";

const DESKTOP_SUFFIX: &str = ".desktop";

pub(crate) struct Described {
    pub(crate) name: String,
    pub(crate) summary: String,
}

pub(crate) fn describe(workspace: &Path, component: &Component) -> Result<Described> {
    match component.about {
        About::Text { name, summary } => Ok(Described {
            name: name.to_owned(),
            summary: summary.to_owned(),
        }),
        About::Entry => {
            let Some(entry) = component.session.data.iter().find(|data| {
                data.into == APPLICATIONS_DIRECTORY && data.source.ends_with(DESKTOP_SUFFIX)
            }) else {
                bail!("{} ships no desktop entry to describe it", component.id);
            };
            let source = workspace.join(entry.source);
            read_entry(&host::read(&source)?)
                .with_context(|| format!("{} lacks Name or Comment", source.display()))
        }
    }
}

fn read_entry(text: &str) -> Option<Described> {
    let mut in_group = false;
    let mut name = None;
    let mut summary = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_group = line == GROUP;
            continue;
        }
        if !in_group {
            continue;
        }
        if let Some(value) = line.strip_prefix("Name=") {
            name = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("Comment=") {
            summary = Some(value.to_owned());
        }
    }
    Some(Described {
        name: name?,
        summary: summary?,
    })
}
