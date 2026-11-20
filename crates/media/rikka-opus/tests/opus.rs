use rikka_opus::{OPUS_MAX_PACKET, OPUS_SAMPLE_RATE, OpusApplication, OpusDecoder, OpusEncoder};

const FRAME: usize = OPUS_SAMPLE_RATE as usize / 100;
const FRAMES: usize = 50;
const PITCH: f32 = 440.0;

#[test]
fn opus_packets_decode_back_to_the_tone_and_conceal_a_gap() {
    let mut encoder = OpusEncoder::new(OpusApplication::LowDelay, 2).unwrap();
    encoder.set_bitrate(128_000).unwrap();
    let mut decoder = OpusDecoder::new(2).unwrap();
    let mut packet = vec![0; OPUS_MAX_PACKET];
    let mut sent = Vec::new();
    let mut heard = Vec::new();
    for index in 0..FRAMES {
        let pcm: Vec<f32> = (0..FRAME)
            .flat_map(|sample| {
                let at = (index * FRAME + sample) as f32 / OPUS_SAMPLE_RATE as f32;
                let level = (at * PITCH * std::f32::consts::TAU).sin() * 0.5;
                [level, level]
            })
            .collect();
        sent.extend_from_slice(&pcm);
        let length = encoder.encode(&pcm, &mut packet).unwrap();
        let frames = match index {
            25 => decoder.conceal(FRAME, &mut heard).unwrap(),
            _ => decoder.decode(&packet[..length], &mut heard).unwrap(),
        };
        assert_eq!(frames, FRAME);
    }
    assert_eq!(heard.len(), sent.len());
    let settled = 10 * FRAME * 2;
    let energy = |samples: &[f32]| samples.iter().map(|sample| sample * sample).sum::<f32>();
    let tail = &heard[settled..];
    let ratio = energy(tail) / energy(&sent[settled..]);
    assert!(
        (0.7..1.3).contains(&ratio),
        "the tone came back at {ratio} of its energy"
    );
    assert!(OpusDecoder::new(3).is_err());
}
