use crate::cff::blend::Blend;
use crate::cff::charstring::Program;
use crate::cff::dict::{op, parse_dict};
use crate::cff::fd_select::FdSelect;
use crate::cff::index::{Index, IndexSpan};
use crate::cff::operands::Operands;
use crate::cff::subfont::{Matrix, Subfonts, matrix_of, to_offset, transform};
use crate::outline::sink::OutlineSink;
use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::glyph_id::GlyphId;
use crate::variations::item_variation_store::ItemVariationStore;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cff2<'a> {
    data: &'a [u8],
    global_subrs: Index<'a>,
    charstrings: Index<'a>,
    fd_array: Index<'a>,
    fd_select: Option<FdSelect<'a>>,
    store: Option<ItemVariationStore<'a>>,
    matrix: Option<Matrix>,
    units_per_em: u16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    global_subrs: IndexSpan,
    charstrings: IndexSpan,
    fd_array: IndexSpan,
    fd_select: Option<usize>,
    store: Option<(usize, usize)>,
    matrix: Option<Matrix>,
}

#[derive(Clone, Copy, Debug, Default)]
struct TopDict {
    charstrings: Option<f64>,
    fd_array: Option<f64>,
    fd_select: Option<f64>,
    store: Option<f64>,
    matrix: Option<Matrix>,
}

impl Layout {
    pub(crate) fn parse(data: &[u8]) -> Option<Self> {
        if data.u8_at(0)? != 2 {
            return None;
        }
        let header_size = usize::from(data.u8_at(2)?);
        let top_length = usize::from(data.u16_at(3)?);
        let top = parse_top(data.range(header_size, top_length)?)?;
        let (global_subrs, _) = IndexSpan::parse_cff2(data, header_size + top_length)?;
        let (charstrings, _) = IndexSpan::parse_cff2(data, to_offset(top.charstrings?)?)?;
        let (fd_array, _) = IndexSpan::parse_cff2(data, to_offset(top.fd_array?)?)?;
        let fd_select = match top.fd_select {
            Some(offset) => {
                let offset = to_offset(offset)?;
                FdSelect::new(data.tail(offset)?)?;
                Some(offset)
            }
            None => None,
        };
        let store = match top.store {
            Some(offset) => {
                let offset = to_offset(offset)?;
                let length = usize::from(data.u16_at(offset)?);
                ItemVariationStore::new(data.range(offset + 2, length)?)?;
                Some((offset + 2, length))
            }
            None => None,
        };
        Some(Self {
            global_subrs,
            charstrings,
            fd_array,
            fd_select,
            store,
            matrix: top.matrix,
        })
    }
}

impl<'a> Cff2<'a> {
    #[inline]
    pub(crate) fn bind(data: &'a [u8], layout: &Layout, units_per_em: u16) -> Option<Self> {
        let fd_select = match layout.fd_select {
            Some(offset) => Some(FdSelect::new(data.tail(offset)?)?),
            None => None,
        };
        let store = match layout.store {
            Some((offset, length)) => Some(ItemVariationStore::new(data.range(offset, length)?)?),
            None => None,
        };
        Some(Self {
            data,
            global_subrs: layout.global_subrs.view(data)?,
            charstrings: layout.charstrings.view(data)?,
            fd_array: layout.fd_array.view(data)?,
            fd_select,
            store,
            matrix: layout.matrix,
            units_per_em,
        })
    }

    pub(crate) fn outline(
        &self,
        glyph: GlyphId,
        coords: &[F2Dot14],
        subfonts: &mut Subfonts,
        operands: &mut Operands,
        sink: &mut impl OutlineSink,
    ) -> Option<()> {
        let charstring = self.charstrings.get(u32::from(glyph.0))?;
        let index = match self.fd_select {
            Some(select) => select.font_dict(glyph)?,
            None => 0,
        };
        let subfont = subfonts.get(self.data, self.fd_array, index, true)?;
        let private = subfont.private.bind(self.data)?;
        let program = Program {
            global_subrs: self.global_subrs,
            local_subrs: private.local_subrs,
            seac: None,
            blend: self.store.map(|store| Blend { store, coords }),
            vsindex: private.vsindex,
            is_cff2: true,
        };
        program.execute(
            charstring,
            transform(self.units_per_em, self.matrix, subfont.matrix),
            sink,
            operands,
        )?;
        Some(())
    }
}

fn parse_top(dict: &[u8]) -> Option<TopDict> {
    let mut top = TopDict::default();
    parse_dict(dict, |operator, operands| {
        let last = operands.last().copied();
        match operator {
            op::CHAR_STRINGS => top.charstrings = last,
            op::FD_ARRAY => top.fd_array = last,
            op::FD_SELECT => top.fd_select = last,
            op::VSTORE => top.store = last,
            op::FONT_MATRIX => top.matrix = matrix_of(operands),
            _ => {}
        }
    })?;
    Some(top)
}
