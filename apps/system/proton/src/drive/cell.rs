use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use proton_files::{Entry, Thumb};
use rikka_app::prelude::*;
use rikka_media_format::{Format, Kind};
use rikka_proton_drive::LinkId;
use rikka_ui::{Element, ImageSource};

use crate::drive::layout::{Column, GUTTER, ROW, TILE, TILE_ART, TILE_LINE, TILE_LINES};
use crate::wording;

pub(crate) struct Shown {
    pub(crate) entries: Arc<Vec<Entry>>,
    pub(crate) thumbs: Arc<HashMap<LinkId, Thumb>>,
    pub(crate) chosen: Option<usize>,
    pub(crate) tiles: bool,
    pub(crate) widths: ColumnWidths,
}

pub(crate) struct Cell {
    pub(crate) shown: Rc<Shown>,
    pub(crate) index: usize,
}

impl Cell {
    pub(crate) fn element(self, height: f32) -> Element {
        let width = match self.shown.tiles {
            true => Length::px(TILE),
            false => Length::FILL,
        };
        Element::new(self).width(width).height(height)
    }

    fn entry(&self) -> Option<&Entry> {
        self.shown.entries.get(self.index)
    }

    fn paint_row(&self, text: &mut CellText, ctx: &mut PaintCtx<'_>) {
        let Some(entry) = self.entry() else {
            return;
        };
        let palette = palette();
        let line = ctx.bounds();
        let widths = &self.shown.widths;
        let selected = self.shown.chosen == Some(self.index);
        text.settle(entry);

        if selected {
            ctx.draw_quad(Quad::new(line).fill(palette.selection));
        } else if self.index % 2 == 1 {
            let mut stripe = palette.surface_variant;
            stripe.a *= 0.45;
            ctx.draw_quad(Quad::new(line).fill(stripe));
        }

        let (symbol, tint) = looks(entry, &palette);
        let icon = Rect::new(
            line.origin.x + COLUMN_PADDING,
            line.origin.y + 4.0,
            20.0,
            20.0,
        );
        ctx.draw_icon(icon, IconGlyph::from(symbol).spec(18.0), tint);

        for (column, which) in Column::ALL.into_iter().enumerate() {
            let (start, width) = (widths.start(column), widths.at(column));
            let gutter = match column == 0 {
                true => GUTTER,
                false => 0.0,
            };
            let room = width - COLUMN_PADDING * 2.0 - gutter;
            let shown = text.fitted(entry, which, room);
            if shown.text.is_empty() {
                continue;
            }
            let x = match which.right_aligned() {
                true => line.origin.x + start + width - COLUMN_PADDING - shown.width,
                false => line.origin.x + start + COLUMN_PADDING + gutter,
            };
            let at = Rect::new(
                x,
                line.origin.y + (ROW - 17.0) / 2.0,
                shown.width + 2.0,
                17.0,
            );
            let tone = match column == 0 {
                true => palette.ink(selected),
                false => palette.text_muted,
            };
            ctx.draw_text(at, TextRun::new(shown.text.as_str(), 13.0), tone);
        }
    }

    fn paint_tile(&self, text: &mut CellText, ctx: &mut PaintCtx<'_>) {
        let Some(entry) = self.entry() else {
            return;
        };
        let palette = palette();
        let tile = ctx.bounds();
        let selected = self.shown.chosen == Some(self.index);

        if selected {
            ctx.draw_quad(
                Quad::new(tile)
                    .fill(palette.selection)
                    .radius(Corner::Item)
                    .stroke(1.0, palette.accent),
            );
        }

        let art = Rect::new(tile.origin.x, tile.origin.y + 6.0, TILE, TILE_ART);
        match self.shown.thumbs.get(&entry.id) {
            Some(thumb) => ctx.draw_image(
                ImageFit::ScaleDown.place(art.inflate(-6.0), thumb.size()),
                ImageSource::Bytes(Arc::clone(&thumb.bytes)),
            ),
            None => {
                let (symbol, tint) = looks(entry, &palette);
                ctx.draw_icon(art, IconGlyph::from(symbol).spec(52.0), tint);
            }
        }

        text.settle(entry);
        for (line, fitted) in text.wrapped(entry).iter().enumerate() {
            let at = Rect::new(
                tile.origin.x + (TILE - fitted.width) / 2.0,
                art.max_y() + 4.0 + line as f32 * TILE_LINE,
                fitted.width + 2.0,
                TILE_LINE,
            );
            ctx.draw_text(
                at,
                TextRun::new(fitted.text.as_str(), 12.0),
                palette.ink(selected),
            );
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CellText {
    at: Option<(LinkId, jiff::Timestamp, u64)>,
    modified: String,
    kind: String,
    size: String,
    fitted: [Option<(f32, FittedLine)>; Column::ALL.len()],
    wrapped: Option<Vec<FittedLine>>,
}

impl CellText {
    fn settle(&mut self, entry: &Entry) {
        let at = Some((entry.id.clone(), entry.modified, rikka_i18n::generation()));
        if self.at == at {
            return;
        }
        self.at = at;
        self.modified = wording::moment(entry.modified);
        self.kind = wording::kind(entry);
        self.size = match entry.folder {
            true => String::new(),
            false => wording::size(entry.size),
        };
        self.fitted = Default::default();
        self.wrapped = None;
    }

    fn fitted(&mut self, entry: &Entry, which: Column, room: f32) -> &FittedLine {
        let text = match which {
            Column::Name => entry.name.as_str(),
            Column::Modified => self.modified.as_str(),
            Column::Kind => self.kind.as_str(),
            Column::Size => self.size.as_str(),
        };
        let slot = &mut self.fitted[which as usize];
        if slot.as_ref().is_none_or(|(width, _)| *width != room) {
            let run = TextRun::new("", 13.0).no_wrap().content(text);
            let fitted = fitted_line(&run, room);
            *slot = Some((room, fitted));
        }
        &slot.as_ref().expect("the column was just fitted").1
    }

    fn wrapped(&mut self, entry: &Entry) -> &[FittedLine] {
        self.wrapped.get_or_insert_with(|| {
            let run = TextRun::new("", 12.0)
                .no_wrap()
                .content(entry.name.as_str());
            fitted_lines(&run, TILE - 8.0, TILE_LINES)
        })
    }
}

impl Widget for Cell {
    type State = CellText;

    fn paint(&self, text: &mut CellText, ctx: &mut PaintCtx<'_>) {
        match self.shown.tiles {
            true => self.paint_tile(text, ctx),
            false => self.paint_row(text, ctx),
        }
    }

    fn behavior(&self, _held: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn debug_name(&self) -> &'static str {
        "DriveCell"
    }
}
fn looks(entry: &Entry, palette: &Palette) -> (MaterialSymbol, Color) {
    if entry.folder {
        return (MaterialSymbol::FOLDER, Color::rgb(0.98, 0.78, 0.36));
    }

    match Format::from_path(&entry.name).map(Format::kind) {
        Some(Kind::Picture) => (MaterialSymbol::IMAGE, palette.success),
        Some(Kind::Sound) => (MaterialSymbol::AUDIO_FILE, palette.warning),
        Some(Kind::Movie) => (MaterialSymbol::MOVIE, palette.accent),
        None => match entry.suffix().unwrap_or_default().to_lowercase().as_str() {
            "pdf" => (MaterialSymbol::PICTURE_AS_PDF, palette.text_muted),
            "zip" | "gz" | "xz" | "zst" | "7z" | "tar" | "bz2" | "rar" => {
                (MaterialSymbol::FOLDER_ZIP, Color::rgb(0.85, 0.71, 0.42))
            }
            "txt" | "md" | "log" => (MaterialSymbol::DRAFT, palette.text_muted),
            _ => (MaterialSymbol::DESCRIPTION, palette.text_muted),
        },
    }
}
