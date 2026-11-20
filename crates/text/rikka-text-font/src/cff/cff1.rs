use std::fmt;

use crate::cff::charset::Charset;
use crate::cff::charstring::{Program, Seac};
use crate::cff::dict::{op, parse_dict};
use crate::cff::fd_select::FdSelect;
use crate::cff::index::{Index, IndexSpan};
use crate::cff::operands::Operands;
use crate::cff::standard_strings::STANDARD_STRINGS;
use crate::cff::subfont::{
    Matrix, PrivateDict, PrivateValues, Subfonts, matrix_of, to_offset, transform,
};
use crate::outline::sink::OutlineSink;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;

const TYPE2_CHARSTRINGS: f64 = 2.0;

#[derive(Clone, Copy)]
pub struct Cff<'a> {
    data: &'a [u8],
    strings: Index<'a>,
    global_subrs: Index<'a>,
    charstrings: Index<'a>,
    charset: Option<Charset<'a>>,
    fonts: Fonts<'a>,
    matrix: Option<Matrix>,
    units_per_em: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    strings: IndexSpan,
    global_subrs: IndexSpan,
    charstrings: IndexSpan,
    fonts: FontsLayout,
    charset: f64,
    matrix: Option<Matrix>,
}

#[derive(Clone, Copy, Debug)]
enum FontsLayout {
    Single(PrivateValues),
    CidKeyed {
        fd_array: IndexSpan,
        fd_select: usize,
    },
}

#[derive(Clone, Copy)]
enum Fonts<'a> {
    Single(PrivateDict<'a>),
    CidKeyed {
        fd_array: Index<'a>,
        fd_select: FdSelect<'a>,
    },
}

struct GlyphRun<'a> {
    width: Option<f64>,
    private: PrivateDict<'a>,
    transform: Option<[f32; 4]>,
}

#[derive(Clone, Copy, Debug, Default)]
struct TopDict {
    charset: f64,
    charstrings: Option<f64>,
    private: Option<(f64, f64)>,
    cid_keyed: bool,
    fd_array: Option<f64>,
    fd_select: Option<f64>,
    matrix: Option<Matrix>,
    charstring_type: Option<f64>,
}

impl Layout {
    pub(crate) fn parse(data: &[u8]) -> Option<Self> {
        if data.u8_at(0)? != 1 {
            return None;
        }
        let header_size = usize::from(data.u8_at(2)?);
        let (_, after_names) = Index::parse_cff(data, header_size)?;
        let (top_dicts, strings_at) = Index::parse_cff(data, after_names)?;
        let (strings, global_subrs_at) = IndexSpan::parse_cff(data, strings_at)?;
        let (global_subrs, _) = IndexSpan::parse_cff(data, global_subrs_at)?;
        let top = parse_top(top_dicts.get(0)?)?;
        if top
            .charstring_type
            .is_some_and(|kind| kind != TYPE2_CHARSTRINGS)
        {
            return None;
        }
        let (charstrings, _) = IndexSpan::parse_cff(data, to_offset(top.charstrings?)?)?;
        let fonts = if top.cid_keyed {
            let (fd_array, _) = IndexSpan::parse_cff(data, to_offset(top.fd_array?)?)?;
            let fd_select = to_offset(top.fd_select?)?;
            FdSelect::new(data.tail(fd_select)?)?;
            FontsLayout::CidKeyed {
                fd_array,
                fd_select,
            }
        } else {
            FontsLayout::Single(PrivateValues::parse(data, top.private, false)?)
        };
        Some(Self {
            strings,
            global_subrs,
            charstrings,
            fonts,
            charset: top.charset,
            matrix: top.matrix,
        })
    }
}

impl<'a> Cff<'a> {
    pub(crate) fn new(data: &'a [u8], units_per_em: u16) -> Option<Self> {
        Self::bind(data, &Layout::parse(data)?, units_per_em)
    }

    #[inline]
    pub(crate) fn bind(data: &'a [u8], layout: &Layout, units_per_em: u16) -> Option<Self> {
        let fonts = match layout.fonts {
            FontsLayout::Single(private) => Fonts::Single(private.bind(data)?),
            FontsLayout::CidKeyed {
                fd_array,
                fd_select,
            } => Fonts::CidKeyed {
                fd_array: fd_array.view(data)?,
                fd_select: FdSelect::new(data.tail(fd_select)?)?,
            },
        };
        let charset = to_offset(layout.charset).and_then(|offset| Charset::new(data, offset));
        Some(Self {
            data,
            strings: layout.strings.view(data)?,
            global_subrs: layout.global_subrs.view(data)?,
            charstrings: layout.charstrings.view(data)?,
            charset,
            fonts,
            matrix: layout.matrix,
            units_per_em,
        })
    }

    #[inline]
    pub fn is_cid_keyed(&self) -> bool {
        matches!(self.fonts, Fonts::CidKeyed { .. })
    }

    #[inline]
    pub fn glyph_count(&self) -> u32 {
        self.charstrings.len()
    }

    pub fn glyph_name(&self, glyph: GlyphId) -> Option<&'a str> {
        if self.is_cid_keyed() {
            return None;
        }
        let sid = usize::from(self.charset?.sid(glyph, self.glyph_count())?);
        match STANDARD_STRINGS.get(sid) {
            Some(name) => Some(name),
            None => {
                std::str::from_utf8(self.strings.get((sid - STANDARD_STRINGS.len()) as u32)?).ok()
            }
        }
    }

    pub fn advance_width(&self, glyph: GlyphId) -> Option<f32> {
        let mut operands = Operands::default();
        let run = self.run(
            glyph,
            &mut Subfonts::default(),
            &mut operands,
            &mut NullSink,
        )?;
        let width = match run.width {
            Some(argument) => run.private.nominal_width + argument,
            None => run.private.default_width,
        };
        let scale = run.transform.map_or(1.0, |[a, ..]| a);
        Some(width as f32 * scale)
    }

    pub(crate) fn outline(
        &self,
        glyph: GlyphId,
        subfonts: &mut Subfonts,
        operands: &mut Operands,
        sink: &mut impl OutlineSink,
    ) -> Option<()> {
        self.run(glyph, subfonts, operands, sink).map(|_| ())
    }

    fn subfont(
        &self,
        glyph: GlyphId,
        subfonts: &mut Subfonts,
    ) -> Option<(PrivateDict<'a>, Option<Matrix>)> {
        match self.fonts {
            Fonts::Single(private) => Some((private, None)),
            Fonts::CidKeyed {
                fd_array,
                fd_select,
            } => {
                let index = fd_select.font_dict(glyph)?;
                let subfont = subfonts.get(self.data, fd_array, index, false)?;
                Some((subfont.private.bind(self.data)?, subfont.matrix))
            }
        }
    }

    fn run(
        &self,
        glyph: GlyphId,
        subfonts: &mut Subfonts,
        operands: &mut Operands,
        sink: &mut impl OutlineSink,
    ) -> Option<GlyphRun<'a>> {
        let charstring = self.charstrings.get(u32::from(glyph.0))?;
        let (private, font_matrix) = self.subfont(glyph, subfonts)?;
        let seac = match (self.fonts, self.charset) {
            (Fonts::Single(_), Some(charset)) => Some(Seac {
                charstrings: self.charstrings,
                charset,
            }),
            _ => None,
        };
        let program = Program {
            global_subrs: self.global_subrs,
            local_subrs: private.local_subrs,
            seac,
            blend: None,
            vsindex: 0,
            is_cff2: false,
        };
        let transform = transform(self.units_per_em, self.matrix, font_matrix);
        let width = program.execute(charstring, transform, sink, operands)?;
        Some(GlyphRun {
            width,
            private,
            transform,
        })
    }
}

fn parse_top(dict: &[u8]) -> Option<TopDict> {
    let mut top = TopDict::default();
    parse_dict(dict, |operator, operands| {
        let last = operands.last().copied();
        match operator {
            op::CHARSET => top.charset = last.unwrap_or(0.0),
            op::CHAR_STRINGS => top.charstrings = last,
            op::PRIVATE => {
                if let [size, offset] = operands {
                    top.private = Some((*size, *offset));
                }
            }
            op::ROS => top.cid_keyed = true,
            op::FD_ARRAY => top.fd_array = last,
            op::FD_SELECT => top.fd_select = last,
            op::FONT_MATRIX => top.matrix = matrix_of(operands),
            op::CHARSTRING_TYPE => top.charstring_type = last,
            _ => {}
        }
    })?;
    Some(top)
}

struct NullSink;

impl OutlineSink for NullSink {
    fn move_to(&mut self, _: f32, _: f32) {}
    fn line_to(&mut self, _: f32, _: f32) {}
    fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn cubic_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
    fn close(&mut self) {}
}

impl fmt::Debug for Cff<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cff")
            .field("glyph_count", &self.glyph_count())
            .field("cid_keyed", &self.is_cid_keyed())
            .finish_non_exhaustive()
    }
}

impl<'a> Font<'a> {
    pub fn cff(&self) -> Option<Cff<'a>> {
        Cff::new(self.table_data(TableId::Cff)?, self.units_per_em())
    }
}
