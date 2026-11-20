use rikka_encoding::{Decoder, SHIFT_JIS, UTF_8, UTF_16LE, decode};

#[test]
fn ladder_prefers_mark_then_declaration_then_utf8_then_guess() {
    let marked = decode(b"\xFF\xFEa\x00".as_slice());
    assert_eq!(
        (marked.text.as_str(), marked.encoding, marked.bom),
        ("a", UTF_16LE, true)
    );

    let plain = decode("桜".as_bytes());
    assert_eq!(
        (plain.text.as_str(), plain.encoding, plain.bom),
        ("桜", UTF_8, false)
    );

    let shift_jis = SHIFT_JIS.encode("日本語のファイル名です").0.into_owned();
    let declared = Decoder::new().declared(SHIFT_JIS).decode(shift_jis.clone());
    assert_eq!(declared.text, "日本語のファイル名です");
    let guessed = decode(shift_jis);
    assert_eq!(
        (guessed.text.as_str(), guessed.encoding),
        ("日本語のファイル名です", SHIFT_JIS)
    );
}
