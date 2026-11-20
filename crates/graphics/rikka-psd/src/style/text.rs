use crate::color::Color;
use crate::descriptor::Value;
use crate::engine::EngineValue;
use crate::error::{Error, Result};
use crate::info::{Info, TypeTool};
use crate::layers::Layer;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    pub font: Option<String>,
    pub size: Option<f64>,
    pub color: Option<Color>,
    pub faux_bold: Option<bool>,
    pub faux_italic: Option<bool>,
    pub leading: Option<f64>,
    pub auto_leading: Option<bool>,
    pub tracking: Option<f64>,
    pub kerning: Option<f64>,
    pub horizontal_scale: Option<f64>,
    pub vertical_scale: Option<f64>,
    pub baseline_shift: Option<f64>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub length: usize,
    pub style: TextStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justification {
    Left,
    Right,
    Center,
    JustifyLastLeft,
    JustifyLastRight,
    JustifyLastCenter,
    JustifyAll,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphRun {
    pub length: usize,
    pub justification: Justification,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayer {
    pub tool: TypeTool,
    pub engine: EngineValue,
}

impl TextLayer {
    pub fn from_layer(layer: &Layer) -> Option<Result<Self>> {
        layer.info.iter().find_map(|info| match info {
            Info::TypeTool(tool) => Some(Self::from_tool(tool.clone())),
            _ => None,
        })
    }

    pub fn from_tool(tool: TypeTool) -> Result<Self> {
        let bytes = tool.text.bytes("EngineData").ok_or(Error::Malformed {
            what: "text layer without engine data",
            offset: 0,
        })?;
        let engine = EngineValue::parse(bytes)?;
        Ok(Self { tool, engine })
    }

    pub fn text(&self) -> String {
        let raw = self
            .engine
            .path(&["EngineDict", "Editor", "Text"])
            .and_then(EngineValue::as_str)
            .map(str::to_owned)
            .or_else(|| self.tool.text.string("Txt ").map(str::to_owned))
            .unwrap_or_default();
        raw.strip_suffix('\r').unwrap_or(&raw).replace('\r', "\n")
    }

    pub fn transform(&self) -> [f64; 6] {
        self.tool.transform
    }

    pub fn fonts(&self) -> Vec<String> {
        self.engine
            .path(&["ResourceDict", "FontSet"])
            .and_then(EngineValue::as_array)
            .unwrap_or(&[])
            .iter()
            .map(|font| {
                font.get("Name")
                    .and_then(EngineValue::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect()
    }

    fn style(&self, data: &EngineValue) -> TextStyle {
        let fonts = self.fonts();
        let number = |key: &str| data.get(key).and_then(EngineValue::number);
        let flag = |key: &str| data.get(key).and_then(EngineValue::as_bool);
        let color = data
            .get("FillColor")
            .and_then(|fill| fill.get("Values"))
            .and_then(EngineValue::as_array)
            .and_then(|values| {
                let channels: Vec<f64> = values.iter().filter_map(EngineValue::number).collect();
                (channels.len() == 4).then(|| Color::Rgb {
                    r: channels[1],
                    g: channels[2],
                    b: channels[3],
                })
            });
        TextStyle {
            font: number("Font").and_then(|index| fonts.get(index as usize).cloned()),
            size: number("FontSize"),
            color,
            faux_bold: flag("FauxBold"),
            faux_italic: flag("FauxItalic"),
            leading: number("Leading"),
            auto_leading: flag("AutoLeading"),
            tracking: number("Tracking"),
            kerning: number("Kerning"),
            horizontal_scale: number("HorizontalScale"),
            vertical_scale: number("VerticalScale"),
            baseline_shift: number("BaselineShift"),
            underline: flag("Underline"),
            strikethrough: flag("Strikethrough"),
        }
    }

    pub fn default_style(&self) -> TextStyle {
        self.engine
            .path(&[
                "EngineDict",
                "StyleRun",
                "DefaultRunData",
                "StyleSheet",
                "StyleSheetData",
            ])
            .map(|data| self.style(data))
            .unwrap_or_default()
    }

    pub fn style_runs(&self) -> Vec<TextRun> {
        let Some(style_run) = self.engine.path(&["EngineDict", "StyleRun"]) else {
            return Vec::new();
        };
        let lengths = run_lengths(style_run);
        let runs = style_run
            .get("RunArray")
            .and_then(EngineValue::as_array)
            .unwrap_or(&[]);
        lengths
            .into_iter()
            .zip(runs)
            .map(|(length, run)| TextRun {
                length,
                style: run
                    .path(&["StyleSheet", "StyleSheetData"])
                    .map(|data| self.style(data))
                    .unwrap_or_default(),
            })
            .collect()
    }

    pub fn paragraph_runs(&self) -> Vec<ParagraphRun> {
        let Some(paragraph_run) = self.engine.path(&["EngineDict", "ParagraphRun"]) else {
            return Vec::new();
        };
        let lengths = run_lengths(paragraph_run);
        let runs = paragraph_run
            .get("RunArray")
            .and_then(EngineValue::as_array)
            .unwrap_or(&[]);
        lengths
            .into_iter()
            .zip(runs)
            .map(|(length, run)| {
                let justification = match run
                    .path(&["ParagraphSheet", "Properties", "Justification"])
                    .and_then(EngineValue::number)
                    .unwrap_or(0.0) as i64
                {
                    1 => Justification::Right,
                    2 => Justification::Center,
                    3 => Justification::JustifyLastLeft,
                    4 => Justification::JustifyLastRight,
                    5 => Justification::JustifyLastCenter,
                    6 => Justification::JustifyAll,
                    _ => Justification::Left,
                };
                ParagraphRun {
                    length,
                    justification,
                }
            })
            .collect()
    }

    pub fn set_text(&mut self, text: &str) {
        let content = format!("{}\r", text.replace("\r\n", "\r").replace('\n', "\r"));
        let length = content.encode_utf16().count();
        if let Some(editor) = self.engine.path_mut(&["EngineDict", "Editor"]) {
            editor.set("Text", EngineValue::String(content.clone()));
        }
        if let Some(style_run) = self.engine.path_mut(&["EngineDict", "StyleRun"]) {
            collapse_runs(style_run, vec![length]);
        }
        let paragraphs: Vec<usize> = content
            .split_inclusive('\r')
            .map(|paragraph| paragraph.encode_utf16().count())
            .collect();
        if let Some(paragraph_run) = self.engine.path_mut(&["EngineDict", "ParagraphRun"]) {
            collapse_runs(paragraph_run, paragraphs);
        }
        self.tool.text.set("Txt ", content.trim_end_matches('\r'));
    }

    pub fn set_font_size(&mut self, size: f64) {
        self.edit_styles(|data| data.set("FontSize", EngineValue::Real(size)));
    }

    pub fn set_color(&mut self, color: Color) {
        let [r, g, b] = color.to_rgb();
        let fill = EngineValue::Dict(vec![
            ("Type".to_owned(), EngineValue::Integer(1)),
            (
                "Values".to_owned(),
                EngineValue::Array(vec![
                    EngineValue::Real(1.0),
                    EngineValue::Real(r),
                    EngineValue::Real(g),
                    EngineValue::Real(b),
                ]),
            ),
        ]);
        self.edit_styles(|data| data.set("FillColor", fill.clone()));
    }

    fn edit_styles(&mut self, mut edit: impl FnMut(&mut EngineValue)) {
        if let Some(runs) = self
            .engine
            .path_mut(&["EngineDict", "StyleRun", "RunArray"])
            .and_then(EngineValue::as_array_mut)
        {
            for run in runs {
                if let Some(data) = run.path_mut(&["StyleSheet", "StyleSheetData"]) {
                    edit(data);
                }
            }
        }
    }

    pub fn to_info(&self) -> Info {
        let mut tool = self.tool.clone();
        tool.text
            .set("EngineData", Value::RawData(self.engine.to_bytes()));
        Info::TypeTool(tool)
    }
}

fn run_lengths(run: &EngineValue) -> Vec<usize> {
    run.get("RunLengthArray")
        .and_then(EngineValue::as_array)
        .unwrap_or(&[])
        .iter()
        .map(|value| value.number().unwrap_or(0.0).max(0.0) as usize)
        .collect()
}

fn collapse_runs(run: &mut EngineValue, lengths: Vec<usize>) {
    let first = run
        .get("RunArray")
        .and_then(EngineValue::as_array)
        .and_then(|items| items.first().cloned());
    let Some(first) = first else {
        return;
    };
    run.set("RunArray", EngineValue::Array(vec![first; lengths.len()]));
    run.set(
        "RunLengthArray",
        EngineValue::Array(
            lengths
                .into_iter()
                .map(|length| EngineValue::Integer(length as i64))
                .collect(),
        ),
    );
}
