use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::widget::{Widget, WidgetId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Desks {
    layouts: BTreeMap<Option<String>, Layout>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParseReport {
    pub desks: Desks,
    pub skipped: Vec<Skipped>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub wallpaper: Option<String>,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid desk file: {0}")]
pub struct ParseError(String);

#[derive(Serialize)]
struct Document<'a> {
    desk: Vec<Page<'a>>,
}

#[derive(Serialize)]
struct Page<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    wallpaper: Option<&'a str>,
    widget: &'a [Widget],
}

#[derive(Deserialize)]
struct Read {
    #[serde(default)]
    desk: Vec<ReadPage>,
}

#[derive(Deserialize)]
struct ReadPage {
    #[serde(default)]
    wallpaper: Option<String>,
    #[serde(default)]
    widget: Vec<toml::Value>,
}

impl Desks {
    pub fn parse(text: &str) -> Result<ParseReport, ParseError> {
        let read: Read = toml::from_str(text).map_err(|error| ParseError(error.to_string()))?;
        let mut parsed = ParseReport::default();
        for page in read.desk {
            let wallpaper = page.wallpaper.filter(|id| !id.is_empty());
            let mut known = HashSet::new();
            let mut layout = Layout::default();
            for value in page.widget {
                match Widget::deserialize(value) {
                    Ok(widget) if known.insert(widget.id) => {
                        layout.widgets.push(widget.sanitized());
                    }
                    Ok(widget) => parsed.skipped.push(Skipped {
                        wallpaper: wallpaper.clone(),
                        reason: format!("widget {} appears twice", widget.id.0),
                    }),
                    Err(error) => parsed.skipped.push(Skipped {
                        wallpaper: wallpaper.clone(),
                        reason: error.to_string(),
                    }),
                }
            }
            parsed.desks.layouts.insert(wallpaper, layout);
        }
        Ok(parsed)
    }

    #[must_use]
    pub fn document(&self) -> String {
        let document = Document {
            desk: self
                .layouts
                .iter()
                .map(|(wallpaper, layout)| Page {
                    wallpaper: wallpaper.as_deref(),
                    widget: &layout.widgets,
                })
                .collect(),
        };
        toml::to_string(&document).expect("desk layout serializes to TOML")
    }

    #[must_use]
    pub fn layout(&self, wallpaper: Option<&str>) -> Option<&Layout> {
        self.layouts.get(&wallpaper.map(str::to_owned))
    }

    pub fn layout_mut(&mut self, wallpaper: Option<&str>) -> &mut Layout {
        self.layouts
            .entry(wallpaper.map(str::to_owned))
            .or_default()
    }

    pub fn arrive(&mut self, wallpaper: Option<&str>, from: Option<&str>) -> bool {
        let key = wallpaper.map(str::to_owned);
        if self.layouts.contains_key(&key) {
            return false;
        }
        let copied = self.layout(from).cloned().unwrap_or_default();
        self.layouts.insert(key, copied);
        true
    }

    #[must_use]
    pub fn fresh_id(&self) -> WidgetId {
        let highest = self
            .layouts
            .values()
            .flat_map(|layout| layout.widgets.iter().map(|widget| widget.id.0))
            .max();
        WidgetId(highest.map_or(1, |id| id + 1))
    }
}
