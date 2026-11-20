use crate::cff::cff1;
use crate::cff::cff2;
use crate::cff::operands::Operands;
use crate::cff::subfont::Subfonts;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;

const FINGERPRINT_SIZE: usize = 64;

#[derive(Debug, Default)]
pub(crate) struct CffCache {
    table: Option<TableKey>,
    layout: Option<Layout>,
    subfonts: Subfonts,
    operands: Operands,
}

pub(crate) struct Resolved<'a, 'c> {
    pub(crate) table: &'a [u8],
    pub(crate) layout: Layout,
    pub(crate) subfonts: &'c mut Subfonts,
    pub(crate) operands: &'c mut Operands,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Layout {
    Cff(cff1::Layout),
    Cff2(cff2::Layout),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TableKey {
    address: usize,
    len: usize,
    fingerprint: [u8; FINGERPRINT_SIZE],
}

impl CffCache {
    pub(crate) fn resolve<'a>(&mut self, font: &Font<'a>) -> Option<Resolved<'a, '_>> {
        let (table, cff2) = match font.table_data(TableId::Cff2) {
            Some(table) => (table, true),
            None => (font.table_data(TableId::Cff)?, false),
        };
        if !self.table.is_some_and(|key| key.matches(table)) {
            self.table = Some(TableKey::new(table));
            self.layout = if cff2 {
                cff2::Layout::parse(table).map(Layout::Cff2)
            } else {
                cff1::Layout::parse(table).map(Layout::Cff)
            };
            self.subfonts.clear();
            self.operands.regions.invalidate();
        }
        Some(Resolved {
            table,
            layout: self.layout?,
            subfonts: &mut self.subfonts,
            operands: &mut self.operands,
        })
    }
}

impl TableKey {
    fn new(table: &[u8]) -> Self {
        let mut fingerprint = [0; FINGERPRINT_SIZE];
        let prefix = &table[..table.len().min(FINGERPRINT_SIZE)];
        fingerprint[..prefix.len()].copy_from_slice(prefix);
        Self {
            address: table.as_ptr() as usize,
            len: table.len(),
            fingerprint,
        }
    }

    #[inline]
    fn matches(&self, table: &[u8]) -> bool {
        let prefix = table.len().min(FINGERPRINT_SIZE);
        self.address == table.as_ptr() as usize
            && self.len == table.len()
            && self.fingerprint[..prefix] == table[..prefix]
    }
}
