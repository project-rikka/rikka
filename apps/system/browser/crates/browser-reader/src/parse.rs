use serde_json::{Map, Value};

use crate::article::{Article, Direction};
use crate::block::{Block, Cell, Code, Figure, Image, List, Table};
use crate::error::{Error, Limit, Result};
use crate::inline::{Inline, Marks, Run};
use crate::reading::Reading;
use crate::url::{ImageSource, Link};

const MAX_BLOCKS: usize = 8_000;
const MAX_TEXT: usize = 6 * 1024 * 1024;
const MAX_DATA_IMAGES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 8;
const MAX_IMAGES: usize = 1_000;
const MAX_INLINES: usize = 20_000;
const MAX_ROWS: usize = 1_000;
const MAX_CELLS: usize = 64;
const MAX_SPAN: u64 = 64;
const MAX_DIMENSION: u64 = 100_000;
const MAX_TITLE: usize = 2_000;
const MAX_LINE: usize = 500;
const MAX_LANGUAGE: usize = 35;
const MAX_CODE_LANGUAGE: usize = 32;

pub fn parse(answer: &Value) -> Result<Article> {
    let fields = answer.as_object().ok_or(Error::Malformed("answer"))?;
    if !readable_flag(fields)? {
        return Err(Error::NotReadable);
    }
    let mut budget = Budget::default();
    let title = line(fields.get("title"), MAX_TITLE, "title", &mut budget)?
        .ok_or(Error::Malformed("title"))?;
    let byline = line(fields.get("byline"), MAX_LINE, "byline", &mut budget)?;
    let site = line(fields.get("site"), MAX_LINE, "site", &mut budget)?;
    let published = line(fields.get("published"), MAX_LINE, "published", &mut budget)?;
    let language = line(fields.get("lang"), MAX_LANGUAGE, "lang", &mut budget)?
        .filter(|language| valid_language(language));
    let direction = match fields.get("dir").and_then(Value::as_str) {
        Some(direction) if direction.eq_ignore_ascii_case("rtl") => Direction::RightToLeft,
        _ => Direction::LeftToRight,
    };
    let lead = match fields.get("image") {
        None | Some(Value::Null) => None,
        Some(image) => budget.image(image)?,
    };
    let blocks = budget.blocks(fields.get("blocks"), 0)?;
    if blocks.is_empty() {
        return Err(Error::NotReadable);
    }
    let truncated = optional_flag(fields, "truncated")?;
    let reading = Reading::measure(&blocks);
    Ok(Article {
        title,
        byline,
        site,
        published,
        language,
        direction,
        lead,
        blocks,
        truncated,
        reading,
    })
}

pub fn readable(answer: &Value) -> Result<bool> {
    let fields = answer.as_object().ok_or(Error::Malformed("answer"))?;
    readable_flag(fields)
}

fn readable_flag(fields: &Map<String, Value>) -> Result<bool> {
    fields
        .get("readable")
        .and_then(Value::as_bool)
        .ok_or(Error::Malformed("readable"))
}

fn optional_flag(fields: &Map<String, Value>, name: &'static str) -> Result<bool> {
    match fields.get(name) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(flag)) => Ok(*flag),
        Some(_) => Err(Error::Malformed(name)),
    }
}

fn optional_string<'a>(value: Option<&'a Value>, name: &'static str) -> Result<Option<&'a str>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(Error::Malformed(name)),
    }
}

fn line(
    value: Option<&Value>,
    limit: usize,
    name: &'static str,
    budget: &mut Budget,
) -> Result<Option<String>> {
    let Some(text) = optional_string(value, name)? else {
        return Ok(None);
    };
    budget.consume(text.len())?;
    let collapsed = text
        .split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let clipped = collapsed.chars().take(limit).collect::<String>();
    Ok((!clipped.is_empty()).then_some(clipped))
}

fn code_text(text: &str) -> String {
    text.chars()
        .filter(|&character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}

fn run_text(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn valid_language(language: &str) -> bool {
    let mut parts = language.split('-');
    parts.next().is_some_and(|primary| {
        (2..=8).contains(&primary.len()) && primary.bytes().all(|byte| byte.is_ascii_alphabetic())
    }) && parts.all(|part| {
        (1..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

fn valid_code_language(language: &str) -> bool {
    !language.is_empty()
        && language.len() <= MAX_CODE_LANGUAGE
        && language
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+#._-".contains(&byte))
}

fn array<'a>(value: Option<&'a Value>, name: &'static str) -> Result<&'a [Value]> {
    match value {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(Error::Malformed(name)),
    }
}

fn object<'a>(value: &'a Value, name: &'static str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or(Error::Malformed(name))
}

#[derive(Default)]
struct Budget {
    blocks: usize,
    text: usize,
    data_images: usize,
    images: usize,
}

impl Budget {
    fn consume(&mut self, bytes: usize) -> Result<()> {
        self.text += bytes;
        if self.text > MAX_TEXT {
            return Err(Error::TooLarge(Limit::Text));
        }
        Ok(())
    }

    fn blocks(&mut self, value: Option<&Value>, depth: usize) -> Result<Vec<Block>> {
        if depth > MAX_DEPTH {
            return Err(Error::TooLarge(Limit::Nesting));
        }
        let items = array(value, "blocks")?;
        let mut blocks = Vec::with_capacity(items.len().min(MAX_BLOCKS));
        for item in items {
            self.blocks += 1;
            if self.blocks > MAX_BLOCKS {
                return Err(Error::TooLarge(Limit::Blocks));
            }
            if let Some(block) = self.block(item, depth)? {
                blocks.push(block);
            }
        }
        Ok(blocks)
    }

    fn block(&mut self, value: &Value, depth: usize) -> Result<Option<Block>> {
        let fields = object(value, "block")?;
        let kind = fields
            .get("type")
            .and_then(Value::as_str)
            .ok_or(Error::Malformed("block type"))?;
        Ok(match kind {
            "heading" => {
                let level = fields
                    .get("level")
                    .and_then(Value::as_u64)
                    .ok_or(Error::Malformed("heading level"))?
                    .clamp(1, 6) as u8;
                let content = self.inlines(fields.get("content"))?;
                (!content.is_empty()).then_some(Block::Heading { level, content })
            }
            "paragraph" => {
                let content = self.inlines(fields.get("content"))?;
                (!content.is_empty()).then_some(Block::Paragraph(content))
            }
            "list" => self.list(fields, depth)?,
            "quote" => {
                let blocks = self.blocks(fields.get("blocks"), depth + 1)?;
                (!blocks.is_empty()).then_some(Block::Quote(blocks))
            }
            "code" => self.code(fields)?,
            "figure" => self.figure(fields)?,
            "table" => self.table(fields)?,
            "rule" => Some(Block::Rule),
            _ => return Err(Error::Malformed("block type")),
        })
    }

    fn list(&mut self, fields: &Map<String, Value>, depth: usize) -> Result<Option<Block>> {
        let ordered = optional_flag(fields, "ordered")?;
        let start = match fields.get("start") {
            None | Some(Value::Null) => 1,
            Some(start) => start
                .as_i64()
                .ok_or(Error::Malformed("list start"))?
                .clamp(0, i64::from(u32::MAX)) as u32,
        };
        let mut items = Vec::new();
        for item in array(fields.get("items"), "list items")? {
            if !item.is_array() {
                return Err(Error::Malformed("list item"));
            }
            let blocks = self.blocks(Some(item), depth + 1)?;
            if !blocks.is_empty() {
                items.push(blocks);
            }
        }
        Ok((!items.is_empty()).then_some(Block::List(List {
            ordered,
            start,
            items,
        })))
    }

    fn code(&mut self, fields: &Map<String, Value>) -> Result<Option<Block>> {
        let text = optional_string(fields.get("text"), "code text")?
            .ok_or(Error::Malformed("code text"))?;
        self.consume(text.len())?;
        let text = code_text(text);
        let text = text.trim_end().trim_start_matches(['\n', '\r']).to_owned();
        let language = optional_string(fields.get("language"), "code language")?
            .map(str::trim)
            .filter(|language| valid_code_language(language))
            .map(str::to_owned);
        Ok((!text.is_empty()).then_some(Block::Code(Code { text, language })))
    }

    fn figure(&mut self, fields: &Map<String, Value>) -> Result<Option<Block>> {
        let mut images = Vec::new();
        for image in array(fields.get("images"), "figure images")? {
            if let Some(image) = self.image(image)? {
                images.push(image);
            }
        }
        let caption = self.inlines(fields.get("caption"))?;
        Ok(if images.is_empty() {
            (!caption.is_empty()).then_some(Block::Paragraph(caption))
        } else {
            Some(Block::Figure(Figure { images, caption }))
        })
    }

    fn image(&mut self, value: &Value) -> Result<Option<Image>> {
        let fields = object(value, "image")?;
        self.images += 1;
        if self.images > MAX_IMAGES {
            return Err(Error::TooLarge(Limit::Images));
        }
        let source = optional_string(fields.get("src"), "image source")?
            .ok_or(Error::Malformed("image source"))?;
        let Some(source) = ImageSource::parse(source) else {
            return Ok(None);
        };
        if source.is_data() {
            self.data_images += source.as_str().len();
            if self.data_images > MAX_DATA_IMAGES {
                return Err(Error::TooLarge(Limit::Images));
            }
        } else {
            self.consume(source.as_str().len())?;
        }
        let alt = line(fields.get("alt"), MAX_LINE, "image alt", self)?.unwrap_or_default();
        let dimension = |name: &'static str| -> Result<Option<u32>> {
            match fields.get(name) {
                None | Some(Value::Null) => Ok(None),
                Some(value) => {
                    let number = value.as_f64().ok_or(Error::Malformed(name))?;
                    Ok((1.0..=MAX_DIMENSION as f64)
                        .contains(&number)
                        .then_some(number.round() as u32))
                }
            }
        };
        let width = dimension("width")?;
        let height = dimension("height")?;
        let (width, height) = match (width, height) {
            (Some(width), Some(height)) => (Some(width), Some(height)),
            _ => (None, None),
        };
        Ok(Some(Image {
            source,
            alt,
            width,
            height,
        }))
    }

    fn table(&mut self, fields: &Map<String, Value>) -> Result<Option<Block>> {
        let caption = self.inlines(fields.get("caption"))?;
        let source_rows = array(fields.get("rows"), "table rows")?;
        if source_rows.len() > MAX_ROWS {
            return Err(Error::TooLarge(Limit::Rows));
        }
        let mut rows = Vec::with_capacity(source_rows.len());
        for row in source_rows {
            let Value::Array(source_cells) = row else {
                return Err(Error::Malformed("table row"));
            };
            if source_cells.len() > MAX_CELLS {
                return Err(Error::TooLarge(Limit::Cells));
            }
            let mut cells = Vec::with_capacity(source_cells.len());
            for cell in source_cells {
                cells.push(self.cell(cell)?);
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
        }
        let filled = rows
            .iter()
            .flatten()
            .any(|cell: &Cell| !cell.content.is_empty());
        Ok(filled.then_some(Block::Table(Table { caption, rows })))
    }

    fn cell(&mut self, value: &Value) -> Result<Cell> {
        let fields = object(value, "table cell")?;
        let span = |name: &'static str| -> Result<u16> {
            match fields.get(name) {
                None | Some(Value::Null) => Ok(1),
                Some(value) => Ok(value
                    .as_u64()
                    .ok_or(Error::Malformed(name))?
                    .clamp(1, MAX_SPAN) as u16),
            }
        };
        Ok(Cell {
            header: optional_flag(fields, "header")?,
            columns: span("colspan")?,
            rows: span("rowspan")?,
            content: self.inlines(fields.get("content"))?,
        })
    }

    fn inlines(&mut self, value: Option<&Value>) -> Result<Vec<Inline>> {
        let items = array(value, "inline content")?;
        if items.len() > MAX_INLINES {
            return Err(Error::TooLarge(Limit::Inlines));
        }
        let mut content: Vec<Inline> = Vec::with_capacity(items.len());
        for item in items {
            let fields = object(item, "inline")?;
            match fields.get("type").and_then(Value::as_str) {
                Some("break") => {
                    if content.last().is_some_and(|last| *last != Inline::Break) {
                        content.push(Inline::Break);
                    }
                }
                Some("text") => {
                    if let Some(run) = self.run(fields)? {
                        content.push(Inline::Text(run));
                    }
                }
                _ => return Err(Error::Malformed("inline type")),
            }
        }
        while content.last() == Some(&Inline::Break) {
            content.pop();
        }
        let visible = content.iter().any(|inline| match inline {
            Inline::Text(run) => !run.text.trim().is_empty(),
            Inline::Break => false,
        });
        Ok(if visible { content } else { Vec::new() })
    }

    fn run(&mut self, fields: &Map<String, Value>) -> Result<Option<Run>> {
        let text = optional_string(fields.get("text"), "inline text")?
            .ok_or(Error::Malformed("inline text"))?;
        self.consume(text.len())?;
        let text = run_text(text);
        if text.is_empty() {
            return Ok(None);
        }
        let marks = Marks {
            emphasis: optional_flag(fields, "em")?,
            strong: optional_flag(fields, "strong")?,
            code: optional_flag(fields, "code")?,
            superscript: optional_flag(fields, "sup")?,
            subscript: optional_flag(fields, "sub")?,
            deleted: optional_flag(fields, "del")?,
        };
        let link = match optional_string(fields.get("href"), "inline link")? {
            Some(href) => {
                self.consume(href.len())?;
                Link::parse(href)
            }
            None => None,
        };
        let ruby = line(fields.get("ruby"), MAX_LINE, "ruby", self)?;
        Ok(Some(Run {
            text,
            marks,
            link,
            ruby,
        }))
    }
}
