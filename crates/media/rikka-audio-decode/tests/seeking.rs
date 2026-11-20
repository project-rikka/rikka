use crate::tones;

const TARGET: u64 = 12_345;
const SPAN: usize = 4_000;

#[test]
fn seek_is_sample_accurate() {
    let cases: &[(&str, &[u8], f32)] = &[
        ("flac", tones::FLAC, 0.0),
        ("wavpack", tones::WAVPACK, 0.0),
        ("mp3", tones::MP3, 0.02),
        ("vorbis", tones::VORBIS, 0.02),
        ("opus", tones::OPUS, 0.02),
        ("aac", tones::AAC, 0.02),
    ];
    for (name, bytes, tolerance) in cases {
        let whole = rikka_audio_decode::read(*bytes).decode().unwrap();
        let mut decoder = rikka_audio_decode::read(*bytes).decoder().unwrap();
        assert_eq!(decoder.seek_frame(TARGET).unwrap(), TARGET, "{name}");
        let mut streamed = Vec::new();
        while streamed.len() < SPAN * 2 && decoder.read(&mut streamed).unwrap() {}
        let from = TARGET as usize * 2;
        let expected = &whole.samples()[from..from + SPAN * 2];
        let difference = tones::rms_difference(&streamed[..SPAN * 2], expected);
        assert!(difference <= *tolerance, "{name}: {difference}");

        let mut rest = streamed.len() / 2;
        let mut more = Vec::new();
        while decoder.read(&mut more).unwrap() {}
        rest += more.len() / 2;
        assert_eq!(
            rest as u64,
            whole.frames() - TARGET,
            "{name} ends where it should"
        );
    }
}

#[test]
fn seek_past_end_ends() {
    let mut decoder = rikka_audio_decode::read(tones::OPUS).decoder().unwrap();
    assert_eq!(decoder.seek_frame(u64::MAX).unwrap(), tones::FRAMES);
    let mut samples = Vec::new();
    while decoder.read(&mut samples).unwrap() {}
    assert!(samples.is_empty());
}
