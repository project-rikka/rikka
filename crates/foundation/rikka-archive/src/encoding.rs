pub(crate) fn decode_name(raw: &[u8]) -> String {
    rikka_encoding::decode(raw).text
}
