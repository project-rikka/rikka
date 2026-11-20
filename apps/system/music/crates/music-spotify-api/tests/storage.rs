use std::time::{Duration, UNIX_EPOCH};

use music_spotify_api::CdnUrl;

const SIGNED_AT: u64 = 1_688_165_560;
const MARGIN: Duration = Duration::from_secs(5 * 60);

#[test]
fn cdn_urls_expire_by_their_signature() {
    let expiry = UNIX_EPOCH + Duration::from_secs(SIGNED_AT) - MARGIN;
    for (url, expires) in [
        (
            format!(
                "https://audio-cf.spotifycdn.com/audio/844ecdb297a87ebfee4399f28892ef85d9ba725f?verify={SIGNED_AT}-4R3I2w2q7OfNkR%2FGH8qH7xtIKUPlDxywBuADY%2BsvMeU%3D"
            ),
            Some(expiry),
        ),
        (
            format!(
                "https://audio-ak-spotify-com.akamaized.net/audio/foo?__token__=exp={SIGNED_AT}~hmac=4e661527574fab5793adb99cf04e1c2ce12294c71fe1d39ffbfabdcfe8ce3b41"
            ),
            Some(expiry),
        ),
        (
            format!(
                "https://audio-gm-off.spotifycdn.com/audio/foo?Expires={SIGNED_AT}~FullPath~hmac=IIZA28qptl8cuGLq15-SjHKHtLoxzpy_6r_JpAU4MfM="
            ),
            Some(expiry),
        ),
        (
            format!(
                "https://audio4-fa.scdn.co/audio/foo?{SIGNED_AT}_0GKSyXjLaTW1BksFOyI4J7Tf9tZDbBUNNPu9Mt4mhH4="
            ),
            Some(expiry),
        ),
        ("https://audio4-fa.scdn.co/foo?baz".to_owned(), None),
    ] {
        let parsed = CdnUrl::parse(&url);
        assert_eq!(parsed.expires, expires, "{url}");
        if let Some(expires) = expires {
            assert!(parsed.is_live(expires - Duration::from_secs(1)), "{url}");
            assert!(!parsed.is_live(expires), "{url}");
        }
    }
}
