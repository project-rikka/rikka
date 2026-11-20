use crate::inline::Inline;
use crate::url::ImageSource;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, content: Vec<Inline> },
    Paragraph(Vec<Inline>),
    List(List),
    Quote(Vec<Block>),
    Code(Code),
    Figure(Figure),
    Table(Table),
    Rule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    pub ordered: bool,
    pub start: u32,
    pub items: Vec<Vec<Block>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub text: String,
    pub language: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Figure {
    pub images: Vec<Image>,
    pub caption: Vec<Inline>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub source: ImageSource,
    pub alt: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub caption: Vec<Inline>,
    pub rows: Vec<Vec<Cell>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub header: bool,
    pub content: Vec<Inline>,
    pub columns: u16,
    pub rows: u16,
}

impl Block {
    pub(crate) fn each_text(&self, visit: &mut impl FnMut(&str)) {
        match self {
            Block::Heading { content, .. } | Block::Paragraph(content) => {
                inline_text(content, visit)
            }
            Block::List(list) => list
                .items
                .iter()
                .flatten()
                .for_each(|block| block.each_text(visit)),
            Block::Quote(blocks) => blocks.iter().for_each(|block| block.each_text(visit)),
            Block::Code(code) => visit(&code.text),
            Block::Figure(figure) => inline_text(&figure.caption, visit),
            Block::Table(table) => {
                inline_text(&table.caption, visit);
                table
                    .rows
                    .iter()
                    .flatten()
                    .for_each(|cell| inline_text(&cell.content, visit));
            }
            Block::Rule => {}
        }
    }

    pub(crate) fn each_image(&self, visit: &mut impl FnMut(&Image)) {
        match self {
            Block::Figure(figure) => figure.images.iter().for_each(&mut *visit),
            Block::List(list) => list
                .items
                .iter()
                .flatten()
                .for_each(|block| block.each_image(visit)),
            Block::Quote(blocks) => blocks.iter().for_each(|block| block.each_image(visit)),
            _ => {}
        }
    }
}

fn inline_text(content: &[Inline], visit: &mut impl FnMut(&str)) {
    for inline in content {
        if let Inline::Text(run) = inline {
            visit(&run.text);
        }
    }
}
