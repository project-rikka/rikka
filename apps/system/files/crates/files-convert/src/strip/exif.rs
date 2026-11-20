const ORIENTATION: u16 = 0x0112;
const SHORT: u16 = 3;
const ENTRY: usize = 12;
const ORIENTATION_NORMAL: u16 = 1;
const ORIENTATION_MAX: u16 = 8;

pub(crate) const JPEG_PREFIX: &[u8] = b"Exif\0\0";

pub(crate) fn orientation(tiff: &[u8]) -> Option<u16> {
    let tiff = tiff.strip_prefix(JPEG_PREFIX).unwrap_or(tiff);
    let big = match tiff.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let half = |at: usize| -> Option<u16> {
        let bytes: [u8; 2] = tiff.get(at..at + 2)?.try_into().ok()?;
        Some(if big {
            u16::from_be_bytes(bytes)
        } else {
            u16::from_le_bytes(bytes)
        })
    };
    let word = |at: usize| -> Option<u32> {
        let bytes: [u8; 4] = tiff.get(at..at + 4)?.try_into().ok()?;
        Some(if big {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    };
    if half(2)? != 42 {
        return None;
    }
    let directory = usize::try_from(word(4)?).ok()?;
    let entries = usize::from(half(directory)?);
    (0..entries)
        .find_map(|index| {
            let entry = directory + 2 + index * ENTRY;
            let turned = half(entry)? == ORIENTATION && half(entry + 2)? == SHORT;
            turned.then(|| half(entry + 8)).flatten()
        })
        .filter(|value| (ORIENTATION_NORMAL + 1..=ORIENTATION_MAX).contains(value))
}

pub(crate) fn only_orientation(orientation: u16) -> Vec<u8> {
    let mut tiff = Vec::with_capacity(26);
    tiff.extend_from_slice(b"MM");
    tiff.extend_from_slice(&42u16.to_be_bytes());
    tiff.extend_from_slice(&8u32.to_be_bytes());
    tiff.extend_from_slice(&1u16.to_be_bytes());
    tiff.extend_from_slice(&ORIENTATION.to_be_bytes());
    tiff.extend_from_slice(&SHORT.to_be_bytes());
    tiff.extend_from_slice(&1u32.to_be_bytes());
    tiff.extend_from_slice(&orientation.to_be_bytes());
    tiff.extend_from_slice(&[0, 0]);
    tiff.extend_from_slice(&0u32.to_be_bytes());
    tiff
}
