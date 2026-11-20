use rikka_ui::text::{Label, TextStyled, text};
use rikka_ui::{Align, Element, ImageSource, Inset, Length, Styled, View, font, hash_of};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use pulldown_cmark::{Alignment, BlockQuoteKind};
use rikka_text::TextSpan;
use rikka_text::Weight;
use rikka_theme::Syntax;
use rikka_types::Color;

use crate::build::{Builder, Kind};
use crate::inline::{Piece, StyledText};
use rikka_symbols::MaterialSymbol;
use rikka_text::Align as TextAlign;
use rikka_ui::text::selection::Part;
use rikka_widgets::{
    ImageFit, RichText, column_of, container, copy_button, deferred, grid_of, icon, image,
    rich_text, row_of, spacer,
};

const IMAGE_HEIGHT: f32 = 200.0;

const HEADING_LEADING: f32 = 1.25;

const STRIPE_ALPHA: f32 = 0.4;

const HEADER_INSET: f32 = 1.0 / 3.0;

impl Builder<'_> {
    fn shown(&self, content: Arc<str>) -> RichText {
        let widget = rich_text(content);
        match self.unveils() {
            true => widget.unveiling(),
            false => widget,
        }
    }

    fn part(&self, content: impl Into<Arc<str>>) -> RichText {
        let content = content.into();
        let widget = self.shown(Arc::clone(&content));
        let widget = match self.found() {
            Some(found) => widget.found(found),
            None => widget,
        };
        match self.gather() {
            Some(gather) => widget.part(gather.add(&content)),
            None => widget,
        }
    }

    pub(super) fn prose(
        &self,
        pieces: Vec<Piece>,
        size: f32,
        weight: Weight,
        color: Option<Color>,
    ) -> Element {
        let leading = match weight == font::BOLD {
            true => (size * HEADING_LEADING).round(),
            false => self.rendering.style.line_height,
        };
        self.opens();
        let mut parts: Vec<Element> = pieces
            .into_iter()
            .map(|piece| match piece {
                Piece::Text(styled) => self.styled_text(styled, size, leading, weight, color),
                Piece::Image { url, alt } => self.picture(&url, &alt),
            })
            .collect();
        match parts.len() {
            1 => parts.remove(0),
            _ => row_of(parts).wrap_items().gap(size / 3.0).build(),
        }
    }

    fn styled_text(
        &self,
        styled: StyledText,
        size: f32,
        leading: f32,
        weight: Weight,
        color: Option<Color>,
    ) -> Element {
        let StyledText {
            text,
            spans,
            fills,
            links,
            targets,
        } = styled;
        let mut widget = self
            .part(text)
            .spans(spans)
            .fills(fills)
            .text_size(size)
            .weight(weight)
            .line_height(leading)
            .family(self.rendering.style.family.clone());
        if let Some(color) = color {
            widget = widget.color(color);
        }
        if !links.is_empty() {
            widget = widget.links(links);
            if let Some(on_link) = &self.rendering.on_link {
                widget = widget.on_link(on_link.clone().map(move |(index,): (usize,)| {
                    (targets.get(index).cloned().unwrap_or_default(),)
                }));
            }
        }
        widget.build()
    }

    pub(super) fn heading(&self, pieces: Vec<Piece>, level: usize) -> Element {
        let size = self.rendering.style.heading_size(level);
        let prose = self.prose(pieces, size, font::BOLD, None);
        match level <= 2 {
            true => column_of([prose, self.rule()])
                .gap(self.rendering.style.padding / 2.0)
                .build(),
            false => prose,
        }
    }

    pub(super) fn quote(&self, kind: Option<BlockQuoteKind>, blocks: Vec<Element>) -> Element {
        let palette = self.rendering.palette;
        let color = match kind {
            Some(BlockQuoteKind::Note | BlockQuoteKind::Important) => palette.accent,
            Some(BlockQuoteKind::Tip) => palette.success,
            Some(BlockQuoteKind::Warning) => palette.warning,
            Some(BlockQuoteKind::Caution) => palette.danger,
            None => palette.outline,
        };
        let mut content = blocks;
        if let Some(kind) = kind {
            content.insert(0, self.alert_title(kind, color));
        }
        let bar = container(spacer().width(Length::px(self.rendering.style.quote_bar)))
            .background(color)
            .radius(self.rendering.style.quote_bar / 2.0);
        rikka_widgets::row! {
            bar;
            column_of(content).gap(self.rendering.style.block_gap / 2.0).grow(1.0)
        }
        .gap(self.rendering.style.padding)
        .align_items(Align::Stretch)
        .build()
    }

    fn alert_title(&self, kind: BlockQuoteKind, color: Color) -> Element {
        let (symbol, key, english) = match kind {
            BlockQuoteKind::Note => (MaterialSymbol::INFO, "markdown.alert.note", "Note"),
            BlockQuoteKind::Tip => (MaterialSymbol::LIGHTBULB, "markdown.alert.tip", "Tip"),
            BlockQuoteKind::Important => (
                MaterialSymbol::PRIORITY_HIGH,
                "markdown.alert.important",
                "Important",
            ),
            BlockQuoteKind::Warning => {
                (MaterialSymbol::WARNING, "markdown.alert.warning", "Warning")
            }
            BlockQuoteKind::Caution => {
                (MaterialSymbol::REPORT, "markdown.alert.caution", "Caution")
            }
        };
        rikka_widgets::row! {
            icon(symbol).extent(self.rendering.style.text_size * 1.15).color(color);
            text(Label::translated_or(key, english))
                .text_size(self.rendering.style.text_size)
                .semibold()
                .color(color)
        }
        .gap(self.rendering.style.padding / 2.0)
        .align_items(Align::Center)
        .build()
    }

    pub(super) fn list(&self, items: Vec<Element>) -> Element {
        column_of(items)
            .gap(self.rendering.style.block_gap / 3.0)
            .build()
    }

    pub(super) fn item(&self, task: Option<bool>, blocks: Vec<Element>) -> Element {
        let gap = self.rendering.style.padding / 2.0;
        let marker: Element = match task {
            Some(checked) => icon(match checked {
                true => MaterialSymbol::CHECK_BOX,
                false => MaterialSymbol::CHECK_BOX_OUTLINE_BLANK,
            })
            .extent(self.rendering.style.text_size * 1.1)
            .color(match checked {
                true => self.rendering.palette.accent,
                false => self.rendering.palette.text_muted,
            })
            .build(),
            None => rich_text(self.bullet())
                .text_size(self.rendering.style.text_size)
                .color(self.rendering.palette.text_muted)
                .line_height(self.rendering.style.line_height)
                .align(TextAlign::Right)
                .build(),
        };
        rikka_widgets::row! {
            container(marker)
                .width(Length::px((self.rendering.style.list_indent - gap).max(0.0)))
                .align_x(Align::End);
            column_of(blocks).gap(self.rendering.style.block_gap / 2.0).grow(1.0)
        }
        .gap(gap)
        .build()
    }

    fn bullet(&self) -> String {
        let Some(Kind::List { start }) = self.stack.last().map(|frame| &frame.kind) else {
            return "•".to_owned();
        };
        match start {
            Some(first) => {
                let index = self.stack.last().map_or(0, |frame| frame.blocks.len());
                format!("{}.", first + index as u64)
            }
            None => "•".to_owned(),
        }
    }

    pub(super) fn code_block(&self, code: &str, speech: Option<&str>) -> Element {
        let code = code.trim_end_matches('\n');
        self.opens();
        let whole = self.gather().map(|gather| gather.add(code));
        let spans: Arc<[TextSpan]> = code_spans(self.rendering.syntax, code, speech).into();
        let source: Arc<str> = Arc::from(code);
        let styled = match code.lines().count() > CODE_CHUNK {
            true => self.code_chunks(code, &spans, whole.as_ref()),
            false => self
                .code_run(Arc::clone(&source), spans, whole)
                .width(Length::FILL)
                .build(),
        };
        let padding = self.rendering.style.padding;
        let body = container(styled)
            .padding(Inset::edges(0.0, padding, padding, padding))
            .width(Length::FILL)
            .build();
        container(column_of([self.code_header(source, speech), body]).width(Length::FILL))
            .background(self.rendering.palette.surface_variant)
            .radius(self.rendering.style.radius)
            .width(Length::FILL)
            .build()
    }

    fn code_header(&self, source: Arc<str>, speech: Option<&str>) -> Element {
        let padding = self.rendering.style.padding;
        let inset = padding * HEADER_INSET;
        rikka_widgets::row! {
            if let Some(language) = speech {
                text(language)
                    .text_size(self.rendering.style.code_size())
                    .family(self.rendering.style.code_family.clone())
                    .color(self.rendering.palette.text_muted)
                    .no_wrap();
            }
            spacer().grow(1.0);
            copy_button(source).labeled();
        }
        .align_items(Align::Center)
        .padding(Inset::edges(inset, inset, 0.0, padding))
        .width(Length::FILL)
        .build()
    }

    fn code_chunks(&self, code: &str, spans: &Arc<[TextSpan]>, whole: Option<&Part>) -> Element {
        let leading = (self.rendering.style.code_size() * 1.45).round();
        let chunks: Vec<Element> = chunked(code)
            .map(|(from, chunk)| {
                let range = from..from + chunk.len();
                let part = whole.map(|whole| whole.subrange(range.clone()));
                let text: Arc<str> = Arc::from(chunk);
                let spans = rebased(spans, &range);
                let lines = chunk.lines().count().max(1) as f32;
                let (family, size, found) = (
                    self.rendering.style.code_family.clone(),
                    self.rendering.style.code_size(),
                    self.found(),
                );
                let unveils = self.unveils();
                let digest = (from, hash_of(&*text), found.is_some());
                deferred(digest, lines * leading, move || {
                    let mut run = rich_text(Arc::clone(&text))
                        .spans(Arc::clone(&spans))
                        .family(family.clone())
                        .text_size(size)
                        .line_height(leading)
                        .width(Length::FILL);
                    if let Some(part) = part.clone() {
                        run = run.part(part);
                    }
                    if let Some(found) = found.clone() {
                        run = run.found(found);
                    }
                    if unveils {
                        run = run.unveiling();
                    }
                    run.build()
                })
                .build()
            })
            .collect();
        column_of(chunks).width(Length::FILL).build()
    }

    fn code_run(&self, code: Arc<str>, spans: Arc<[TextSpan]>, part: Option<Part>) -> RichText {
        let mut run = self
            .shown(code)
            .spans(spans)
            .family(self.rendering.style.code_family.clone())
            .text_size(self.rendering.style.code_size())
            .line_height((self.rendering.style.code_size() * 1.45).round());
        if let Some(part) = part {
            run = run.part(part);
        }
        match self.found() {
            Some(found) => run.found(found),
            None => run,
        }
    }

    pub(super) fn raw_block(&self, html: &str) -> Element {
        self.opens();
        self.part(html.trim_end())
            .family(self.rendering.style.code_family.clone())
            .text_size(self.rendering.style.code_size())
            .color(self.rendering.palette.text_muted)
            .build()
    }

    pub(super) fn rule(&self) -> Element {
        container(spacer().height(Length::px(self.rendering.style.rule)))
            .background(self.rendering.palette.outline)
            .build()
    }

    pub(super) fn footnote(&self, label: &str, blocks: Vec<Element>) -> Element {
        rikka_widgets::row! {
            rich_text(format!("[{label}]"))
                .text_size(self.rendering.style.code_size())
                .color(self.rendering.palette.text_muted);
            column_of(blocks).gap(self.rendering.style.block_gap / 2.0).grow(1.0)
        }
        .gap(self.rendering.style.padding / 2.0)
        .build()
    }

    pub(super) fn definitions(&self, blocks: Vec<Element>) -> Element {
        column_of(blocks)
            .gap(self.rendering.style.block_gap / 3.0)
            .build()
    }

    pub(super) fn definition(&self, blocks: Vec<Element>) -> Element {
        column_of(blocks)
            .gap(self.rendering.style.block_gap / 2.0)
            .padding(Inset::edges(
                0.0,
                0.0,
                0.0,
                self.rendering.style.list_indent,
            ))
            .build()
    }

    pub(super) fn cell(&self, blocks: Vec<Element>) -> Element {
        let column_index = self.stack.last().map_or(0, |frame| frame.blocks.len());
        let alignment = self
            .stack
            .iter()
            .rev()
            .find_map(|frame| match &frame.kind {
                Kind::Table { alignments, .. } => alignments.get(column_index).copied(),
                _ => None,
            })
            .unwrap_or(Alignment::None);
        container(column_of(blocks).gap(self.rendering.style.block_gap / 2.0))
            .padding(Inset::xy(
                self.rendering.style.padding,
                self.rendering.style.padding / 1.5,
            ))
            .align_x(match alignment {
                Alignment::Center => Align::Center,
                Alignment::Right => Align::End,
                Alignment::Left | Alignment::None => Align::Start,
            })
            .build()
    }

    pub(super) fn table(
        &self,
        alignments: &[Alignment],
        head: usize,
        cells: Vec<Element>,
    ) -> Element {
        let columns = alignments.len().max(1);
        let header = self.rendering.palette.surface_variant;
        let stripe = header.scale_alpha(STRIPE_ALPHA);
        let plated: Vec<Element> = cells
            .into_iter()
            .enumerate()
            .map(|(index, cell)| {
                let fill = match index < head {
                    true => Some(header),
                    false => ((index - head) / columns % 2 == 1).then_some(stripe),
                };
                match fill {
                    Some(fill) => container(cell).background(fill).build(),
                    None => cell,
                }
            })
            .collect();
        container(grid_of(columns, plated))
            .border(self.rendering.style.rule, self.rendering.palette.outline)
            .radius(self.rendering.style.radius)
            .build()
    }

    fn picture(&self, url: &str, alt: &str) -> Element {
        let Some(path) = self.local_path(url) else {
            return self
                .part(alt.to_owned())
                .text_size(self.rendering.style.text_size)
                .color(self.rendering.palette.text_muted)
                .italic()
                .build();
        };
        image(ImageSource::Path(Arc::from(path)))
            .fit(ImageFit::Contain)
            .width(Length::FILL)
            .height(Length::px(IMAGE_HEIGHT))
            .build()
    }

    fn local_path(&self, url: &str) -> Option<PathBuf> {
        let path = match url.split_once("://") {
            Some(("file", rest)) => PathBuf::from(rest),
            Some(_) => return None,
            None => PathBuf::from(url),
        };
        match (path.is_absolute(), self.rendering.base.as_deref()) {
            (false, Some(base)) => Some(base.join(path)),
            (false, None) => None,
            (true, _) => Some(path),
        }
    }
}

fn code_spans(colors: Syntax, code: &str, speech: Option<&str>) -> Vec<TextSpan> {
    match speech.and_then(rikka_syntax::Language::named) {
        Some(language) => language.spans(code, colors),
        None => Vec::new(),
    }
}

const CODE_CHUNK: usize = 80;

fn chunked(code: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut at = 0usize;
    let mut lines = code.split('\n').peekable();
    std::iter::from_fn(move || {
        let from = at;
        let mut line_count = 0usize;
        while line_count < CODE_CHUNK && lines.peek().is_some() {
            let line = lines.next().expect("frame stack is non-empty");
            at += line.len() + usize::from(lines.peek().is_some());
            line_count += 1;
        }
        (line_count > 0).then(|| {
            let end = match lines.peek().is_some() {
                true => at.saturating_sub(1),
                false => at,
            };
            (from, &code[from..end])
        })
    })
}

fn rebased(spans: &Arc<[TextSpan]>, range: &Range<usize>) -> Arc<[TextSpan]> {
    spans
        .iter()
        .filter_map(|span| {
            let from = span.range.start.max(range.start);
            let upto = span.range.end.min(range.end);
            (from < upto).then(|| TextSpan {
                range: from - range.start..upto - range.start,
                ..span.clone()
            })
        })
        .collect()
}
