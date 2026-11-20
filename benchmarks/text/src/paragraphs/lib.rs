use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, fontdb};
use parley::fontique::{Blob, Collection, CollectionOptions};
use parley::{FontContext, FontFamily, FontFamilyName, LayoutContext, LineHeight, StyleProperty};
use rikka_text_catalog::{Query, catalog};
use rikka_text_layout::{
    LayoutParams, Paragraph, ParagraphLayout, ShapedParagraph, TextStyle, Wrap,
};
use text_samples::ParagraphCase;

pub const PARAGRAPH_FAMILY: &str = "Inter";
pub const FONT_SIZE: f32 = 15.0;
pub const LINE_HEIGHT: f32 = 21.0;
pub const WIDE: f32 = 640.0;
pub const NARROW: f32 = 410.0;

fn params(width: f32) -> LayoutParams {
    LayoutParams {
        width: Some(width),
        wrap: Wrap::Word,
        ..LayoutParams::new(FONT_SIZE, LINE_HEIGHT)
    }
}

#[derive(Debug)]
pub struct RikkaParagraphs {
    style: TextStyle,
    wide: LayoutParams,
    narrow: LayoutParams,
    layout: ParagraphLayout,
}

impl Default for RikkaParagraphs {
    fn default() -> Self {
        Self::new()
    }
}

impl RikkaParagraphs {
    pub fn new() -> Self {
        Self {
            style: TextStyle::new([PARAGRAPH_FAMILY]),
            wide: params(WIDE),
            narrow: params(NARROW),
            layout: ParagraphLayout::new(),
        }
    }

    pub fn shape(&self, text: &str) -> ShapedParagraph {
        Paragraph::new(text, &self.style).shape()
    }

    pub fn shape_and_lay(&mut self, text: &str) -> usize {
        let shaped = self.shape(text);
        shaped.layout_into(&self.wide, &mut self.layout);
        self.layout.rows().len()
    }

    pub fn lay(&mut self, shaped: &ShapedParagraph, wide: bool) -> usize {
        let params = if wide { &self.wide } else { &self.narrow };
        shaped.layout_into(params, &mut self.layout);
        self.layout.rows().len()
    }

    pub fn font_files(&mut self, text: &str) -> Vec<PathBuf> {
        let catalog = catalog();
        self.shape_and_lay(text);
        let mut faces: BTreeSet<_> = self
            .layout
            .all_glyphs()
            .iter()
            .map(|glyph| glyph.face)
            .collect();
        if let Some(primary) = catalog.query(&Query::new([PARAGRAPH_FAMILY])) {
            faces.insert(primary.face());
        }
        let files: BTreeSet<PathBuf> = faces
            .into_iter()
            .filter_map(|face| catalog.face(face).path().map(Path::to_owned))
            .collect();
        files.into_iter().collect()
    }
}

pub struct CosmicParagraphs {
    system: FontSystem,
    buffer: Buffer,
}

impl CosmicParagraphs {
    pub fn new(font_files: &[PathBuf]) -> Self {
        let mut database = fontdb::Database::new();
        for file in font_files {
            database.load_font_file(file).unwrap_or_else(|error| {
                panic!("cosmic-text cannot load {}: {error}", file.display())
            });
        }
        let locale = catalog()
            .locale()
            .map_or_else(|| "en-US".to_owned(), |locale| locale.language().to_owned());
        let mut system = FontSystem::new_with_locale_and_db(locale, database);
        let mut buffer = Buffer::new(&mut system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        buffer.set_wrap(cosmic_text::Wrap::Word);
        buffer.set_size(Some(WIDE), None);
        Self { system, buffer }
    }

    pub fn shape_and_lay(&mut self, text: &str) -> usize {
        self.buffer.set_size(Some(WIDE), None);
        self.buffer.set_text(
            text,
            &Attrs::new().family(Family::Name(PARAGRAPH_FAMILY)),
            Shaping::Advanced,
            None,
        );
        self.buffer.shape_until_scroll(&mut self.system, false);
        self.buffer.layout_runs().count()
    }

    pub fn lay(&mut self, wide: bool) -> usize {
        self.buffer
            .set_size(Some(if wide { WIDE } else { NARROW }), None);
        self.buffer.shape_until_scroll(&mut self.system, false);
        self.buffer.layout_runs().count()
    }

    pub fn glyph_count(&self) -> usize {
        self.buffer.layout_runs().map(|run| run.glyphs.len()).sum()
    }
}

impl std::fmt::Debug for CosmicParagraphs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CosmicParagraphs").finish_non_exhaustive()
    }
}

pub struct ParleyParagraphs {
    fonts: FontContext,
    layouts: LayoutContext<()>,
    families: Vec<String>,
    layout: parley::Layout<()>,
}

impl ParleyParagraphs {
    pub fn new(font_files: &[PathBuf]) -> Self {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let mut families = vec![PARAGRAPH_FAMILY.to_owned()];
        for file in font_files {
            let data = std::fs::read(file)
                .unwrap_or_else(|error| panic!("parley cannot read {}: {error}", file.display()));
            for (family, _) in collection.register_fonts(Blob::from(data), None) {
                if let Some(name) = collection.family_name(family)
                    && !families.iter().any(|known| known == name)
                {
                    families.push(name.to_owned());
                }
            }
        }
        Self {
            fonts: FontContext {
                collection,
                source_cache: Default::default(),
            },
            layouts: LayoutContext::new(),
            families,
            layout: parley::Layout::new(),
        }
    }

    pub fn shape_and_lay(&mut self, text: &str) -> usize {
        let stack: Vec<FontFamilyName<'_>> = self
            .families
            .iter()
            .map(|name| FontFamilyName::Named(Cow::Borrowed(name)))
            .collect();
        let mut builder = self
            .layouts
            .ranged_builder(&mut self.fonts, text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Owned(
            stack,
        ))));
        builder.push_default(StyleProperty::FontSize(FONT_SIZE));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(LINE_HEIGHT)));
        builder.build_into(&mut self.layout, text);
        self.layout.break_all_lines(Some(WIDE));
        self.layout.len()
    }

    pub fn lay(&mut self, wide: bool) -> usize {
        self.layout
            .break_all_lines(Some(if wide { WIDE } else { NARROW }));
        self.layout.len()
    }
}

impl std::fmt::Debug for ParleyParagraphs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParleyParagraphs")
            .field("families", &self.families)
            .finish_non_exhaustive()
    }
}
