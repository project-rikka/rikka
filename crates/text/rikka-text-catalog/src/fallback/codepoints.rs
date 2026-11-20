use crate::coverage::PAGE_COUNT;
use crate::face::{Face, FaceId};

#[derive(Debug)]
pub(crate) struct CodepointIndex {
    pages: Box<[Box<[FaceId]>]>,
}

impl CodepointIndex {
    pub(crate) fn build(faces: &[Face]) -> Self {
        let mut pages: Vec<Vec<FaceId>> = vec![Vec::new(); PAGE_COUNT];
        for (index, face) in (0u32..).zip(faces) {
            for &number in face.coverage.page_numbers() {
                pages[usize::from(number)].push(FaceId(index));
            }
        }
        Self {
            pages: pages.into_iter().map(Vec::into_boxed_slice).collect(),
        }
    }

    #[inline]
    pub(crate) fn candidates(&self, character: char) -> &[FaceId] {
        self.pages
            .get((character as u32 >> 8) as usize)
            .map_or(&[], |faces| faces)
    }
}
