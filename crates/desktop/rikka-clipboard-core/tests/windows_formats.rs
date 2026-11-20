use rikka_clipboard_core::dib::{self, Rgba};
use rikka_clipboard_core::hdrop;

#[test]
fn dib_v5_round_trips_straight_alpha() {
    let picture = Rgba {
        width: 2,
        height: 2,
        pixels: vec![
            255, 0, 0, 255, 0, 255, 0, 128, //
            0, 0, 255, 0, 10, 20, 30, 40,
        ],
    };
    let encoded = dib::encode_v5(&picture).unwrap();
    assert_eq!(encoded.len(), 124 + 16);
    assert_eq!(dib::decode(&encoded), Some(picture));
}

#[test]
fn bottom_up_rgb_dib_reads_as_opaque_rows_top_first() {
    let mut bytes = vec![0; 40];
    bytes[0..4].copy_from_slice(&40u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&1i32.to_le_bytes());
    bytes[8..12].copy_from_slice(&2i32.to_le_bytes());
    bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&24u16.to_le_bytes());
    bytes.extend_from_slice(&[1, 2, 3, 0]);
    bytes.extend_from_slice(&[4, 5, 6, 0]);
    let decoded = dib::decode(&bytes).unwrap();
    assert_eq!((decoded.width, decoded.height), (1, 2));
    assert_eq!(decoded.pixels, vec![6, 5, 4, 255, 3, 2, 1, 255]);
}

#[test]
fn hdrop_carries_file_uris_as_windows_paths() {
    let uris = vec![
        "file:///C:/Users/Rikka/a%20b.txt".to_owned(),
        "file://server/share/x.png".to_owned(),
        "https://example.com/".to_owned(),
    ];
    let encoded = hdrop::encode(&uris).unwrap();
    assert_eq!(hdrop::decode(&encoded), uris[..2].to_vec());
    assert_eq!(
        hdrop::path_of(&uris[0]).as_deref(),
        Some("C:\\Users\\Rikka\\a b.txt")
    );
    assert_eq!(
        hdrop::path_of(&uris[1]).as_deref(),
        Some("\\\\server\\share\\x.png")
    );
    assert_eq!(hdrop::encode(&uris[2..]), None);
}
