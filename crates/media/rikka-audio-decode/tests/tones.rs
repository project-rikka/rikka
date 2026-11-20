pub(crate) const RATE: u32 = 48_000;
pub(crate) const FRAMES: u64 = 21_000;

pub(crate) const FLAC: &[u8] = include_bytes!("fixtures/tone.flac");
pub(crate) const ALAC: &[u8] = include_bytes!("fixtures/tone-alac.m4a");
pub(crate) const WAVPACK: &[u8] = include_bytes!("fixtures/tone.wv");
pub(crate) const APE: &[u8] = include_bytes!("fixtures/tone.ape");
pub(crate) const MP3: &[u8] = include_bytes!("fixtures/tone.mp3");
pub(crate) const MP2: &[u8] = include_bytes!("fixtures/tone.mp2");
pub(crate) const VORBIS: &[u8] = include_bytes!("fixtures/tone.ogg");
pub(crate) const OPUS: &[u8] = include_bytes!("fixtures/tone.opus");
pub(crate) const AAC: &[u8] = include_bytes!("fixtures/tone.m4a");
pub(crate) const AAC_ITUNES: &[u8] = include_bytes!("fixtures/tone-itunes.m4a");
pub(crate) const HE_AAC: &[u8] = include_bytes!("fixtures/tone-he.m4a");
pub(crate) const HE_AAC_V2: &[u8] = include_bytes!("fixtures/tone-hev2.m4a");
pub(crate) const WMA: &[u8] = include_bytes!("fixtures/tone.wma");
pub(crate) const AC3: &[u8] = include_bytes!("fixtures/tone.ac3");
pub(crate) const EAC3: &[u8] = include_bytes!("fixtures/tone.eac3");

pub(crate) fn reference() -> Vec<f32> {
    let sound = rikka_audio_decode::read(FLAC).decode().unwrap();
    assert_eq!(sound.frames(), FRAMES);
    sound.samples().to_vec()
}

pub(crate) fn lag(reference: &[f32], decoded: &[f32]) -> i64 {
    let left = |samples: &[f32], frame: i64| -> f32 {
        usize::try_from(frame)
            .ok()
            .and_then(|frame| samples.get(frame * 2))
            .copied()
            .unwrap_or(0.0)
    };
    let span = (reference.len().min(decoded.len()) / 2) as i64;
    (-96i64..=96)
        .map(|lag| {
            let score: f64 = (2_000..span - 2_000)
                .map(|frame| f64::from(left(reference, frame) * left(decoded, frame + lag)))
                .sum();
            (lag, score)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(i64::MAX, |(lag, _)| lag)
}

pub(crate) fn rms_difference(a: &[f32], b: &[f32]) -> f32 {
    let count = a.len().min(b.len()).max(1);
    let sum: f32 = a.iter().zip(b).map(|(a, b)| (a - b) * (a - b)).sum();
    (sum / count as f32).sqrt()
}
