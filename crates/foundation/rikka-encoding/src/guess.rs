use chardetng::EncodingDetector;
use encoding_rs::Encoding;

const SEPARATOR: &[u8] = b" ";

#[must_use]
pub fn guess<'a>(
    samples: impl IntoIterator<Item = &'a [u8]>,
    top_level_domain: Option<&str>,
) -> &'static Encoding {
    let mut detector = EncodingDetector::new();
    let mut fed = false;
    for sample in samples {
        if fed {
            detector.feed(SEPARATOR, false);
        }
        detector.feed(sample, false);
        fed = true;
    }
    detector.feed(&[], true);
    detector.guess(top_level_domain.map(str::as_bytes), false)
}
