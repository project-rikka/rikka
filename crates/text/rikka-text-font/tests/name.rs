use rikka_test_fonts::Writer;
use rikka_text_font::{Font, NameId};

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

fn name_table() -> Vec<u8> {
    let strings: [(u16, u16, u16, u16, Vec<u8>); 5] = [
        (1, 0, 0, 4, b"Caf\x8E".to_vec()),
        (3, 1, 0x0409, 1, utf16("Example")),
        (3, 1, 0x0411, 1, utf16("例のフォント")),
        (3, 1, 0x8000, 1, utf16("Beispiel")),
        (3, 1, 0x0409, 2, utf16("Bold")),
    ];
    let language_tag = utf16("de");
    let header_len = 6 + strings.len() * 12 + 2 + 4;
    let mut table = Writer::new();
    table
        .u16(1)
        .u16(strings.len() as u16)
        .u16(header_len as u16);
    let mut offset = 0;
    for (platform, encoding, language, id, text) in &strings {
        table
            .u16(*platform)
            .u16(*encoding)
            .u16(*language)
            .u16(*id)
            .u16(text.len() as u16)
            .u16(offset);
        offset += text.len() as u16;
    }
    table.u16(1).u16(language_tag.len() as u16).u16(offset);
    for (.., text) in &strings {
        table.raw(text);
    }
    table.raw(&language_tag);
    table.finish()
}

#[test]
fn localized_name_prefers_requested_language() {
    let data = crate::support::sfnt(1, &[(b"name", name_table())]);
    let name = Font::new(&data).unwrap().name().unwrap();
    assert_eq!(name.get(NameId::FAMILY).as_deref(), Some("Example"));
    assert_eq!(
        name.localized(NameId::FAMILY, "ja").as_deref(),
        Some("例のフォント")
    );
    assert_eq!(
        name.localized(NameId::FAMILY, "ja-JP").as_deref(),
        Some("例のフォント")
    );
    assert_eq!(
        name.localized(NameId::FAMILY, "de-AT").as_deref(),
        Some("Beispiel")
    );
    assert_eq!(
        name.localized(NameId::FAMILY, "fr").as_deref(),
        Some("Example")
    );
    assert_eq!(
        name.localized(NameId::SUBFAMILY, "ja").as_deref(),
        Some("Bold")
    );
    assert_eq!(name.get(NameId::DESIGNER), None);
}

#[test]
fn mac_roman_names_decode_high_half() {
    let data = crate::support::sfnt(1, &[(b"name", name_table())]);
    let name = Font::new(&data).unwrap().name().unwrap();
    assert_eq!(name.get(NameId::FULL_NAME).as_deref(), Some("Café"));
}

#[test]
fn truncated_name_tables_never_panic() {
    let table = name_table();
    for len in 0..table.len() {
        let data = crate::support::sfnt(1, &[(b"name", table[..len].to_vec())]);
        let font = Font::new(&data).unwrap();
        if let Some(name) = font.name() {
            for record in name.records() {
                let _ = record.to_string();
            }
            let _ = name.localized(NameId::FAMILY, "ja");
        }
    }
}
