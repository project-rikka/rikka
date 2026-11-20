use rikka_text_unicode::EmojiPresentation;

use crate::fallback::cluster::Cluster;
use crate::locale::Locale;
use crate::matching::FaceMatch;
use crate::style::FontStyle;

const INLINE_KEY: usize = 128;

pub(crate) fn with_key<R>(
    primary: &FaceMatch,
    cluster: &Cluster<'_>,
    locale: Option<Locale>,
    use_key: impl FnOnce(&[u8]) -> R,
) -> R {
    let request = primary.request;
    let (style, angle) = match request.style {
        FontStyle::Normal => (0, 0),
        FontStyle::Italic => (1, 0),
        FontStyle::Oblique(angle) => (2, angle.to_bits()),
    };
    let presentation = match cluster.presentation {
        EmojiPresentation::Text => 0,
        EmojiPresentation::Emoji => 1,
    };
    let mut header = [0u8; 33];
    header[..4].copy_from_slice(&primary.face.0.to_le_bytes());
    header[4..8].copy_from_slice(&primary.chain.0.to_le_bytes());
    header[8..12].copy_from_slice(&request.weight.0.to_bits().to_le_bytes());
    header[12..16].copy_from_slice(&request.width.0.to_bits().to_le_bytes());
    header[16..20].copy_from_slice(&angle.to_le_bytes());
    header[20] = style;
    header[21] = cluster.script as u8;
    header[22] = presentation;
    header[23..33].copy_from_slice(&locale.map(|locale| locale.key()).unwrap_or_default());
    let text = cluster.text.as_bytes();
    let length = header.len() + text.len();
    if length <= INLINE_KEY {
        let mut key = [0u8; INLINE_KEY];
        key[..header.len()].copy_from_slice(&header);
        key[header.len()..length].copy_from_slice(text);
        use_key(&key[..length])
    } else {
        let mut key = Vec::with_capacity(length);
        key.extend_from_slice(&header);
        key.extend_from_slice(text);
        use_key(&key)
    }
}
