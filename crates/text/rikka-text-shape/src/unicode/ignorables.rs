#[inline]
pub(crate) fn is_default_ignorable(codepoint: u32) -> bool {
    let plane = codepoint >> 16;
    if plane == 0 {
        match codepoint >> 8 {
            0x00 => codepoint == 0x00AD,
            0x03 => codepoint == 0x034F,
            0x06 => codepoint == 0x061C,
            0x17 => matches!(codepoint, 0x17B4..=0x17B5),
            0x18 => matches!(codepoint, 0x180B..=0x180E),
            0x20 => matches!(codepoint, 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F),
            0xFE => matches!(codepoint, 0xFE00..=0xFE0F | 0xFEFF),
            0xFF => matches!(codepoint, 0xFFF0..=0xFFF8),
            _ => false,
        }
    } else {
        match plane {
            0x01 => matches!(codepoint, 0x1D173..=0x1D17A),
            0x0E => matches!(codepoint, 0xE0000..=0xE0FFF),
            _ => false,
        }
    }
}

#[inline]
pub(crate) fn is_variation_selector(codepoint: u32) -> bool {
    matches!(codepoint, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}
