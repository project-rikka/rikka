use std::ops::RangeInclusive;
use std::sync::OnceLock;

const PAGE_BITS: u32 = 8;
const PAGE_SIZE: usize = 1 << PAGE_BITS;

type Page = Box<[u16; PAGE_SIZE]>;

type Value = Box<dyn Fn(u32) -> u16 + Send + Sync>;

pub(crate) struct CodepointTable {
    ranges: Box<[RangeInclusive<u32>]>,
    default: u16,
    value: Value,
    pages: Box<[OnceLock<Page>]>,
}

impl CodepointTable {
    pub(crate) fn new(
        ranges: impl IntoIterator<Item = RangeInclusive<u32>>,
        default: u16,
        value: impl Fn(u32) -> u16 + Send + Sync + 'static,
    ) -> Self {
        let ranges: Box<[RangeInclusive<u32>]> = ranges.into_iter().collect();
        let page_count = ranges
            .iter()
            .map(|range| (*range.end() >> PAGE_BITS) as usize + 1)
            .max()
            .unwrap_or(0);
        Self {
            ranges,
            default,
            value: Box::new(value),
            pages: (0..page_count).map(|_| OnceLock::new()).collect(),
        }
    }

    fn fill(&self, page: usize) -> Page {
        let first = (page << PAGE_BITS) as u32;
        let mut values = Box::new([self.default; PAGE_SIZE]);
        for (offset, slot) in values.iter_mut().enumerate() {
            let codepoint = first + offset as u32;
            if self.ranges.iter().any(|range| range.contains(&codepoint)) {
                *slot = (self.value)(codepoint);
            }
        }
        values
    }

    #[inline]
    pub(crate) fn get(&self, codepoint: u32) -> u16 {
        let page = (codepoint >> PAGE_BITS) as usize;
        match self.pages.get(page) {
            Some(cell) => cell.get_or_init(|| self.fill(page))[codepoint as usize % PAGE_SIZE],
            None => self.default,
        }
    }
}

impl std::fmt::Debug for CodepointTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodepointTable")
            .field("pages", &self.pages.len())
            .finish_non_exhaustive()
    }
}
