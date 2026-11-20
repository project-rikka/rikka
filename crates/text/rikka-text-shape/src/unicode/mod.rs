pub(crate) mod codepoint_table;
pub(crate) mod combining_class;
pub(crate) mod general_category;
pub(crate) mod ignorables;
pub(crate) mod space_fallback;

#[inline]
pub(crate) fn to_char(codepoint: u32) -> char {
    char::from_u32(codepoint).unwrap_or(char::REPLACEMENT_CHARACTER)
}
