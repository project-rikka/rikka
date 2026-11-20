const ORIENTATION_TAG: u16 = 0x0112;
const SHORT: u16 = 3;
const ENTRY: usize = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    #[default]
    Upright,
    Rotate90,
    Rotate180,
    Rotate270,
    FlipHorizontal,
    FlipVertical,
    Rotate90FlipHorizontal,
    Rotate270FlipHorizontal,
}

impl Orientation {
    #[must_use]
    pub const fn swaps_axes(self) -> bool {
        matches!(
            self,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Rotate90FlipHorizontal
                | Orientation::Rotate270FlipHorizontal
        )
    }

    pub(crate) const fn from_exif_value(value: u16) -> Option<Orientation> {
        Some(match value {
            1 => Orientation::Upright,
            2 => Orientation::FlipHorizontal,
            3 => Orientation::Rotate180,
            4 => Orientation::FlipVertical,
            5 => Orientation::Rotate90FlipHorizontal,
            6 => Orientation::Rotate90,
            7 => Orientation::Rotate270FlipHorizontal,
            8 => Orientation::Rotate270,
            _ => return None,
        })
    }

    pub(crate) fn from_exif(exif: &[u8]) -> Option<Orientation> {
        let big_endian = match exif.first_chunk::<4>()? {
            b"MM\0*" => true,
            b"II*\0" => false,
            _ => return None,
        };
        let short = |at: usize| {
            let bytes = *exif.get(at..)?.first_chunk::<2>()?;
            Some(match big_endian {
                true => u16::from_be_bytes(bytes),
                false => u16::from_le_bytes(bytes),
            })
        };
        let long = |at: usize| {
            let bytes = *exif.get(at..)?.first_chunk::<4>()?;
            Some(match big_endian {
                true => u32::from_be_bytes(bytes),
                false => u32::from_le_bytes(bytes),
            })
        };
        let directory = long(4)? as usize;
        let entries = usize::from(short(directory)?);
        (0..entries)
            .map(|index| directory.checked_add(2 + index * ENTRY))
            .find_map(|entry| {
                let entry = entry?;
                (short(entry)? == ORIENTATION_TAG && short(entry + 2)? == SHORT)
                    .then(|| short(entry + 8))?
            })
            .and_then(Orientation::from_exif_value)
    }
}
