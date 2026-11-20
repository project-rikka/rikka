use std::collections::HashMap;
use std::sync::Arc;

pub(crate) type Page = [u64; 4];

pub(crate) const PAGE_COUNT: usize = 0x1100;
const BMP_PAGES: u16 = 256;
const EMPTY_PAGE: Page = [0; 4];

pub(crate) struct CoverageBuilder {
    pages: Box<[Page]>,
    touched: Vec<u16>,
}

impl CoverageBuilder {
    pub(crate) fn new() -> Self {
        Self {
            pages: vec![EMPTY_PAGE; PAGE_COUNT].into_boxed_slice(),
            touched: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn add(&mut self, codepoint: u32) {
        let Some(page) = self.pages.get_mut((codepoint >> 8) as usize) else {
            return;
        };
        if *page == EMPTY_PAGE {
            self.touched.push((codepoint >> 8) as u16);
        }
        let low = codepoint & 0xFF;
        page[(low >> 6) as usize] |= 1 << (low & 63);
    }

    pub(crate) fn finish(&mut self) -> Vec<(u16, Page)> {
        self.touched.sort_unstable();
        let pages = self
            .touched
            .iter()
            .map(|&number| {
                let page = std::mem::replace(&mut self.pages[usize::from(number)], EMPTY_PAGE);
                (number, page)
            })
            .collect();
        self.touched.clear();
        pages
    }
}

#[derive(Default)]
pub(crate) struct PageTable {
    pages: Vec<Page>,
    slots: HashMap<Page, u32>,
    indexed: usize,
}

impl PageTable {
    pub(crate) fn from_pages(pages: Vec<Page>) -> Self {
        Self {
            pages,
            slots: HashMap::new(),
            indexed: 0,
        }
    }

    pub(crate) fn intern(&mut self, page: Page) -> u32 {
        for (slot, known) in self.pages.iter().enumerate().skip(self.indexed) {
            self.slots.entry(*known).or_insert(slot as u32);
        }
        let slot = *self.slots.entry(page).or_insert(self.pages.len() as u32);
        if slot as usize == self.pages.len() {
            self.pages.push(page);
        }
        self.indexed = self.pages.len();
        slot
    }

    pub(crate) fn page(&self, slot: u32) -> Option<&Page> {
        self.pages.get(slot as usize)
    }

    pub(crate) fn into_pages(self) -> Vec<Page> {
        self.pages
    }

    pub(crate) fn into_store(self) -> Arc<[Page]> {
        self.pages.into()
    }
}

#[derive(Clone)]
pub(crate) struct Coverage {
    bmp: [u64; 4],
    bmp_count: u32,
    pages: Box<[u16]>,
    slots: Box<[u32]>,
    store: Arc<[Page]>,
}

impl Coverage {
    pub(crate) fn new(entries: &[(u16, u32)], store: Arc<[Page]>) -> Self {
        let mut bmp = [0u64; 4];
        let mut bmp_count = 0;
        for &(page, _) in entries {
            if page < BMP_PAGES {
                bmp[usize::from(page >> 6)] |= 1 << (page & 63);
                bmp_count += 1;
            }
        }
        Self {
            bmp,
            bmp_count,
            pages: entries.iter().map(|&(page, _)| page).collect(),
            slots: entries.iter().map(|&(_, slot)| slot).collect(),
            store,
        }
    }

    #[inline]
    pub(crate) fn contains(&self, codepoint: u32) -> bool {
        let number = codepoint >> 8;
        let position = if number < u32::from(BMP_PAGES) {
            let word = (number >> 6) as usize;
            let bit = number & 63;
            if self.bmp[word] >> bit & 1 == 0 {
                return false;
            }
            let below: u32 = self.bmp[..word].iter().map(|bits| bits.count_ones()).sum();
            (below + (self.bmp[word] & ((1u64 << bit) - 1)).count_ones()) as usize
        } else {
            let start = self.bmp_count as usize;
            let Ok(found) = self.pages[start..].binary_search(&(number as u16)) else {
                return false;
            };
            start + found
        };
        let Some(page) = self
            .slots
            .get(position)
            .and_then(|&slot| self.store.get(slot as usize))
        else {
            return false;
        };
        let low = codepoint & 0xFF;
        page[(low >> 6) as usize] >> (low & 63) & 1 != 0
    }

    pub(crate) fn ascii(&self) -> u128 {
        if self.bmp[0] & 1 == 0 {
            return 0;
        }
        self.slots
            .first()
            .and_then(|&slot| self.store.get(slot as usize))
            .map_or(0, |page| u128::from(page[0]) | u128::from(page[1]) << 64)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (u16, &Page)> {
        self.pages
            .iter()
            .zip(&self.slots)
            .filter_map(|(&page, &slot)| Some((page, self.store.get(slot as usize)?)))
    }

    pub(crate) fn page_numbers(&self) -> &[u16] {
        &self.pages
    }

    pub(crate) fn count(&self) -> u32 {
        self.entries()
            .map(|(_, page)| page.iter().map(|bits| bits.count_ones()).sum::<u32>())
            .sum()
    }
}
