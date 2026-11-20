use std::fmt::{self, Write};

use rikka_html::Escaped;

use crate::block::{Block, Cell, Code, Figure, Image, List, Table};
use crate::inline::{Inline, Run};
use crate::url::Link;

pub(crate) struct Markup<'a, W: Write> {
    out: &'a mut W,
    heading_shift: i16,
}

impl<'a, W: Write> Markup<'a, W> {
    pub(crate) fn new(out: &'a mut W, blocks: &[Block]) -> Self {
        let shallowest = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Heading { level, .. } => Some(i16::from(*level)),
                _ => None,
            })
            .min()
            .unwrap_or(2);
        Markup {
            out,
            heading_shift: 2 - shallowest,
        }
    }

    pub(crate) fn blocks(&mut self, blocks: &[Block]) -> fmt::Result {
        for block in blocks {
            self.block(block)?;
        }
        Ok(())
    }

    fn image(&mut self, image: &Image) -> fmt::Result {
        write!(
            self.out,
            "<img src=\"{}\" alt=\"{}\"",
            Escaped(image.source.as_str()),
            Escaped(&image.alt)
        )?;
        if let (Some(width), Some(height)) = (image.width, image.height) {
            write!(self.out, " width=\"{width}\" height=\"{height}\"")?;
        }
        self.out.write_str(" loading=\"lazy\" decoding=\"async\">")
    }

    fn inlines(&mut self, content: &[Inline]) -> fmt::Result {
        let mut open: Option<&Link> = None;
        for inline in content {
            let link = match inline {
                Inline::Text(run) => run.link.as_ref(),
                Inline::Break => None,
            };
            if open != link {
                if open.is_some() {
                    self.out.write_str("</a>")?;
                }
                if let Some(link) = link {
                    write!(self.out, "<a href=\"{}\">", Escaped(link.as_str()))?;
                }
                open = link;
            }
            match inline {
                Inline::Text(run) => self.run(run)?,
                Inline::Break => self.out.write_str("<br>")?,
            }
        }
        if open.is_some() {
            self.out.write_str("</a>")?;
        }
        Ok(())
    }

    fn block(&mut self, block: &Block) -> fmt::Result {
        match block {
            Block::Heading { level, content } => {
                let level = (i16::from(*level) + self.heading_shift).clamp(2, 6);
                write!(self.out, "<h{level}>")?;
                self.inlines(content)?;
                writeln!(self.out, "</h{level}>")
            }
            Block::Paragraph(content) => {
                self.out.write_str("<p>")?;
                self.inlines(content)?;
                self.out.write_str("</p>\n")
            }
            Block::List(list) => self.list(list),
            Block::Quote(blocks) => {
                self.out.write_str("<blockquote>\n")?;
                self.blocks(blocks)?;
                self.out.write_str("</blockquote>\n")
            }
            Block::Code(code) => self.code(code),
            Block::Figure(figure) => self.figure(figure),
            Block::Table(table) => self.table(table),
            Block::Rule => self.out.write_str("<hr>\n"),
        }
    }

    fn list(&mut self, list: &List) -> fmt::Result {
        let tag = if list.ordered { "ol" } else { "ul" };
        write!(self.out, "<{tag}")?;
        if list.ordered && list.start != 1 {
            write!(self.out, " start=\"{}\"", list.start)?;
        }
        self.out.write_str(">\n")?;
        for item in &list.items {
            self.out.write_str("<li>")?;
            match item.as_slice() {
                [Block::Paragraph(content)] => self.inlines(content)?,
                blocks => {
                    self.out.write_char('\n')?;
                    self.blocks(blocks)?;
                }
            }
            self.out.write_str("</li>\n")?;
        }
        writeln!(self.out, "</{tag}>")
    }

    fn code(&mut self, code: &Code) -> fmt::Result {
        self.out.write_str("<pre><code")?;
        if let Some(language) = &code.language {
            write!(self.out, " class=\"language-{}\"", Escaped(language))?;
        }
        writeln!(self.out, ">{}</code></pre>", Escaped(&code.text))
    }

    pub(crate) fn lead(&mut self, image: &Image) -> fmt::Result {
        self.out.write_str("<figure class=\"lead\">\n")?;
        self.image(image)?;
        self.out.write_str("\n</figure>\n")
    }

    fn figure(&mut self, figure: &Figure) -> fmt::Result {
        self.out.write_str("<figure>\n")?;
        for image in &figure.images {
            self.image(image)?;
            self.out.write_char('\n')?;
        }
        if !figure.caption.is_empty() {
            self.out.write_str("<figcaption>")?;
            self.inlines(&figure.caption)?;
            self.out.write_str("</figcaption>\n")?;
        }
        self.out.write_str("</figure>\n")
    }

    fn table(&mut self, table: &Table) -> fmt::Result {
        self.out.write_str("<div class=\"table\"><table>\n")?;
        if !table.caption.is_empty() {
            self.out.write_str("<caption>")?;
            self.inlines(&table.caption)?;
            self.out.write_str("</caption>\n")?;
        }
        self.out.write_str("<tbody>\n")?;
        for row in &table.rows {
            self.out.write_str("<tr>")?;
            for cell in row {
                self.cell(cell)?;
            }
            self.out.write_str("</tr>\n")?;
        }
        self.out.write_str("</tbody></table></div>\n")
    }

    fn cell(&mut self, cell: &Cell) -> fmt::Result {
        let tag = if cell.header { "th" } else { "td" };
        write!(self.out, "<{tag}")?;
        if cell.columns > 1 {
            write!(self.out, " colspan=\"{}\"", cell.columns)?;
        }
        if cell.rows > 1 {
            write!(self.out, " rowspan=\"{}\"", cell.rows)?;
        }
        self.out.write_char('>')?;
        self.inlines(&cell.content)?;
        write!(self.out, "</{tag}>")
    }

    fn run(&mut self, run: &Run) -> fmt::Result {
        let marks = run.marks;
        let tags = [
            (marks.deleted, "del"),
            (marks.strong, "strong"),
            (marks.emphasis, "em"),
            (marks.code, "code"),
            (marks.superscript, "sup"),
            (marks.subscript, "sub"),
        ];
        for (_, tag) in tags.iter().filter(|(on, _)| *on) {
            write!(self.out, "<{tag}>")?;
        }
        match &run.ruby {
            Some(ruby) => write!(
                self.out,
                "<ruby>{}<rp>(</rp><rt>{}</rt><rp>)</rp></ruby>",
                Escaped(&run.text),
                Escaped(ruby)
            )?,
            None => write!(self.out, "{}", Escaped(&run.text))?,
        }
        for (_, tag) in tags.iter().rev().filter(|(on, _)| *on) {
            write!(self.out, "</{tag}>")?;
        }
        Ok(())
    }
}
