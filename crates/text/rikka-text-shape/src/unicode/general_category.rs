use rikka_text_unicode::GeneralCategory;

#[inline]
pub(crate) const fn category_flag(category: GeneralCategory) -> u32 {
    1 << category as u32
}
