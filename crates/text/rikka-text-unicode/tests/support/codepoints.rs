pub(crate) fn parse_codepoints(field: &str) -> String {
    field
        .split_whitespace()
        .map(|hex| {
            u32::from_str_radix(hex, 16)
                .ok()
                .and_then(char::from_u32)
                .unwrap_or_else(|| panic!("invalid code point `{hex}`"))
        })
        .collect()
}
