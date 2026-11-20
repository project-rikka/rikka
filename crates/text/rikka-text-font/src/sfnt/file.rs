use crate::error::{Error, Result};
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::tag::Tag;

const COLLECTION: Tag = Tag::new(b"ttcf");
const TRUETYPE: Tag = Tag::from_u32(0x0001_0000);
const OPENTYPE: Tag = Tag::new(b"OTTO");
const APPLE_TRUETYPE: Tag = Tag::new(b"true");

#[derive(Clone, Copy, Debug)]
pub struct FontFile<'a> {
    data: &'a [u8],
    collection: Option<Array<'a, u32>>,
}

impl<'a> FontFile<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self> {
        let tag = data.tag_at(0).ok_or(Error::UnknownFormat)?;
        match tag {
            COLLECTION => {
                let count = data.u32_at(8).ok_or(Error::Malformed)?;
                let offsets = data
                    .array::<u32>(12, count as usize)
                    .ok_or(Error::Malformed)?;
                Ok(Self {
                    data,
                    collection: Some(offsets),
                })
            }
            TRUETYPE | OPENTYPE | APPLE_TRUETYPE => Ok(Self {
                data,
                collection: None,
            }),
            _ => Err(Error::UnknownFormat),
        }
    }

    #[inline]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    pub fn is_collection(&self) -> bool {
        self.collection.is_some()
    }

    pub fn face_count(&self) -> u32 {
        self.collection.map_or(1, |offsets| offsets.len() as u32)
    }

    pub fn face(&self, index: u32) -> Result<Font<'a>> {
        let count = self.face_count();
        let missing = Error::FaceIndex { index, count };
        let directory = match self.collection {
            Some(offsets) => offsets.get(index as usize).ok_or(missing)?,
            None if index == 0 => 0,
            None => return Err(missing),
        };
        Font::from_directory(self.data, directory as usize, index)
    }

    pub fn faces(&self) -> impl Iterator<Item = Result<Font<'a>>> + use<'a> {
        let file = *self;
        (0..self.face_count()).map(move |index| file.face(index))
    }
}
