use std::time::Duration;

use rikka_audio_decode::{Codec, Error};

use crate::support::generate_wav;

const RATE: u32 = 2_822_400;
const BYTES: usize = 4096 * 2 + 1000;
const BLOCK: usize = 4096;

fn channels() -> [Vec<u8>; 2] {
    [0u8, 1].map(|channel| {
        (0..BYTES)
            .map(|index| (index * 7 + usize::from(channel) * 13 + index / 251) as u8)
            .collect()
    })
}

fn interleaved(channels: &[Vec<u8>]) -> Vec<u8> {
    (0..BYTES)
        .flat_map(|index| channels.iter().map(move |channel| channel[index]))
        .collect()
}

fn dsf(channels: &[Vec<u8>], lsb_first: bool) -> Vec<u8> {
    let blocks = BYTES.div_ceil(BLOCK);
    let data = blocks * BLOCK * channels.len();
    let mut file = Vec::new();
    file.extend_from_slice(b"DSD ");
    file.extend_from_slice(&28u64.to_le_bytes());
    file.extend_from_slice(&((28 + 52 + 12 + data) as u64).to_le_bytes());
    file.extend_from_slice(&0u64.to_le_bytes());
    file.extend_from_slice(b"fmt ");
    file.extend_from_slice(&52u64.to_le_bytes());
    for field in [
        1u32,
        0,
        2,
        channels.len() as u32,
        RATE,
        if lsb_first { 1 } else { 8 },
    ] {
        file.extend_from_slice(&field.to_le_bytes());
    }
    file.extend_from_slice(&((BYTES * 8) as u64).to_le_bytes());
    file.extend_from_slice(&(BLOCK as u32).to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(b"data");
    file.extend_from_slice(&((12 + data) as u64).to_le_bytes());
    for block in 0..blocks {
        for channel in channels {
            let mut run = vec![0u8; BLOCK];
            let start = block * BLOCK;
            let end = (start + BLOCK).min(BYTES);
            for (slot, byte) in run.iter_mut().zip(&channel[start..end]) {
                *slot = if lsb_first {
                    byte.reverse_bits()
                } else {
                    *byte
                };
            }
            file.extend_from_slice(&run);
        }
    }
    file
}

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut chunk = id.to_vec();
    chunk.extend_from_slice(&(body.len() as u64).to_be_bytes());
    chunk.extend_from_slice(body);
    if body.len() % 2 == 1 {
        chunk.push(0);
    }
    chunk
}

fn dff(channels: &[Vec<u8>]) -> Vec<u8> {
    let mut properties = b"SND ".to_vec();
    properties.extend(chunk(b"FS  ", &RATE.to_be_bytes()));
    let mut layout = (channels.len() as u16).to_be_bytes().to_vec();
    layout.extend_from_slice(b"SLFTSRGT");
    properties.extend(chunk(b"CHNL", &layout));
    let mut compression = b"DSD ".to_vec();
    compression.push(14);
    compression.extend_from_slice(b"not compressed");
    properties.extend(chunk(b"CMPR", &compression));
    let mut body = b"DSD ".to_vec();
    body.extend(chunk(b"FVER", &0x0105_0000u32.to_be_bytes()));
    body.extend(chunk(b"PROP", &properties));
    body.extend(chunk(b"DSD ", &interleaved(channels)));
    chunk(b"FRM8", &body)
}

#[test]
fn dsd_reader_yields_msb_first_bitstream() {
    let channels = channels();
    let expected = interleaved(&channels);
    let files = [
        ("dsf lsb first", dsf(&channels, true)),
        ("dsf msb first", dsf(&channels, false)),
        ("dff", dff(&channels)),
    ];
    for (name, file) in files {
        let mut reader = rikka_audio_decode::read(file.as_slice()).dsd().unwrap();
        assert_eq!(
            (reader.rate(), reader.channels(), reader.frames()),
            (RATE, 2, (BYTES * 8) as u64),
            "{name}"
        );
        let mut stream = Vec::new();
        while reader.read(&mut stream).unwrap() {}
        assert!(stream == expected, "{name} streams the bitstream");

        let byte = 5_000u64;
        let at = Duration::from_secs_f64((byte * 8) as f64 / f64::from(RATE));
        let reached = reader.seek(at + Duration::from_nanos(100)).unwrap();
        assert_eq!(reached, at, "{name}");
        let mut tail = Vec::new();
        while reader.read(&mut tail).unwrap() {}
        assert!(
            tail == expected[byte as usize * 2..],
            "{name} streams from the seek"
        );
    }
}

#[test]
fn dsd_decodes_to_pcm_at_one_eighth_rate() {
    let file = dsf(&channels(), true);
    let info = rikka_audio_decode::read(file.as_slice()).probe().unwrap();
    assert_eq!(info.encoding.codec, Codec::Dsd);
    assert_eq!(info.encoding.dsd_rate, Some(RATE));
    assert!(info.encoding.lossless);
    assert_eq!((info.sample_rate, info.channels), (RATE / 8, 2));
    assert_eq!(info.bits_per_sample, Some(1));
    let sound = rikka_audio_decode::read(file.as_slice()).decode().unwrap();
    assert!(
        sound.frames() > 0 && sound.frames() <= BYTES as u64,
        "{}",
        sound.frames()
    );
}

#[test]
fn pcm_is_not_dsd() {
    let wav = generate_wav(44_100, 441);
    assert!(matches!(
        rikka_audio_decode::read(wav).dsd(),
        Err(Error::NotDsd)
    ));
}
