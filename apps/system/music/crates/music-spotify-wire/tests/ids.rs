use music_spotify_wire::{FileId, SpotifyId};

#[test]
fn spotify_ids_convert_between_base62_and_gid() {
    for (base62, base16, gid) in [
        (
            "5sWHDYs0csV6RS48xBl0tH",
            "b39fe8081e1f4c54be38e8d6f9f12bb9",
            [
                179, 159, 232, 8, 30, 31, 76, 84, 190, 56, 232, 214, 249, 241, 43, 185,
            ],
        ),
        (
            "4GNcXTGWmnZ3ySrqvol3o4",
            "9a1b1cfbc6f244569ae0356c77bbe9d8",
            [
                154, 27, 28, 251, 198, 242, 68, 86, 154, 224, 53, 108, 119, 187, 233, 216,
            ],
        ),
        (
            "0000000000000000000000",
            "00000000000000000000000000000000",
            [0; 16],
        ),
    ] {
        let id = SpotifyId::from_base62(base62).unwrap();
        assert_eq!(id, SpotifyId::from_gid(&gid).unwrap());
        assert_eq!(id.to_gid(), gid);
        assert_eq!(id.to_base62(), base62);
        assert_eq!(id.to_base16(), base16);
    }
}

#[test]
fn malformed_ids_are_refused() {
    for text in [
        "!!!!!Ys0csV6RS48xBl0tH",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "aa",
        "ZZZZZZZZZZZZZZZZZZZZZZ",
    ] {
        assert!(SpotifyId::from_base62(text).is_err(), "{text}");
    }
    assert!(SpotifyId::from_gid(&[154, 27, 28, 251]).is_err());
    assert!(FileId::from_bytes(&[]).is_err());
    assert!(FileId::from_bytes(&[0; 21]).is_err());
}
